# Triage outcomes: session list, notices and window chrome

Written against main at 1396fb526 on 2026-11-24. This is a goal file for an unattended run, executed through the
planning system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it;
everything you need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over
anything this file says.

## The goal

Execute the 18 triage outcomes listed under Outline below, as a short stack of batched PRs. Each outcome is recorded in
`TRIAGE_OUTCOMES.md` under the name of its feedback file in `review_feedback_queue/`, and was decided on 2026-11-24 on
the maintainer's fewest-questions triage page (against main c38b93552), or recorded that day under the clearly-settled
rule. The ledger entry's Assessment and the feedback file hold the finding; its Decision and Completion criteria say
what to do, and they bind except where this file's Decisions retarget them. The planner grouped the outcomes from their
ledger entries and did not recheck each finding against current main: line numbers in the Assessments are from c38b93552
and may have drifted, so confirm each finding in current code before changing anything.

### This plan's area, and the 12 others

This plan's area is the desktop app's session list and sidebar, notices and the notification bell, interrupted-upload
notices, upgrade highlights, the session toolbar, animation ticker and theme colors.

This is one of 13 plans that together execute every outcome pending from the 2026-11-24 triage, split by area of code so
they can run concurrently, as the maintainer asked for the previous batch ("yes we definitely need to batch these in a
much smaller set of plans. please try to separate them by area of code so there is a good chance of them running
concurrently.") and confirmed for this one ("doesn't have to be exactly 10. just a reasonable number"). Any of them may
run at the same time as this one. The others, and the areas this plan leaves to them:

- `triage-bookkeeping-nov24`: queue bookkeeping only: the discarded outcomes, with no code or spec change.
- `triage-spec-text-nov24`: prose only: SPEC.md and SPEC_impl.md text (corrections and the new principles the outcomes
  record), user docs pages, CLI help text, and source comments and docstrings that misdescribe shipped behavior.
- `triage-tooling-nov24`: maintainer tooling: release and brick-test tooling under `releasing/`, the CentOS gate, test
  workers, deflake, the drift runner and real-agent tests, agent-screen capture, GUI goldens and docs shots, and the
  scripts under `scripts/`.
- `triage-custom-agents-nov24`: registered custom agents wherever they appear: the launcher's custom-agent handling,
  recents and search, the custom-agent Settings page, the Templates editor and template replay for custom agents, the
  session menu and quick switcher labels, the CLI's and helm's custom-agent validation and template checks, and the
  supervisor's custom-agent pin-correction tests.
- `triage-launcher-nov24`: the session launcher (Enter and button handling, recents, search, Save as template, the
  filled-from marker, its hook and YOLO questions) and the Templates dialog, outside custom-agent handling.
- `triage-settings-nov24`: the Settings dialog outside the custom-agent page: Save and Close, footers, credentials and
  Tensorlake key forms, notifications, agent trust and hook pages and switches, and Settings tests.
- `triage-terminal-nov24`: the terminal path: the desktop app's terminal view (selection, paging, scrollback, final
  screen, shortcuts during composition), the grid model and link-disclosure code, the vendored terminal crates, and
  their tests.
- `triage-supervisor-nov24`: the supervisor's service: Stop, Restart, Delete, tabs, teardown and sweeps, recovery and
  re-adoption, scope cleanups and its store, plus the e2e suite and its fixtures.
- `triage-hooks-trust-nov24`: persistent hook setup and its hints and warnings, workspace trust (Claude, Codex,
  Antigravity, Pi and the others) on launches and approval cards, and the trust and hook hint bands above the terminal.
- `triage-helm-nov24`: the helm's provider-neutral code and the Hosts panel: SSH and Tailscale hosts, setup and
  provisioning, automatic and manual updates and their sign-in questions, agent requests and their approvals, credential
  delivery, and the CLI.
- `triage-podman-nov24`: the Podman provider: container builds, recipe images and their proxy settings, probes, claims
  and host identity, leftovers and removal, and the container launcher and Settings surfaces.
- `triage-environments-nov24`: Tensorlake sandboxes and the provider-neutral environment code: waking a host at launch,
  Suspend, environment Delete and its late notices, Trash, sandbox leftovers in Settings, and Replace with's fresh-clone
  warning.

Do not change code in those areas unless an outcome of this plan genuinely requires it. When one does, keep the edit to
what the outcome needs, log the overlap as a DECISION, and name it in the report.

### What each kind of outcome asks for

The kind is read from the outcome's ledger Decision; the outline tags each outcome with it.

- **Fix, clearly settled** (Decision: "Recorded without asking under the clearly-settled rule"): change the code so it
  matches the SPEC.md or SPEC_impl.md rule the Assessment quotes, with a test where practical.
- **Fix as decided** (Decision names a question on the 2026-11-24 triage page and the maintainer's pick, sometimes with
  their comment): build exactly what the Completion criteria describe. A `fix spec+code` outcome also makes the spec
  change its criteria name. Where the Decision quotes the maintainer's own words, they are part of the criteria.
- **Approved as listed** (Decision: q-listed, "Approve as listed"): apply the item's own recommendation, quoted in its
  Completion criteria, as written. Where the recommendation offers an alternative ("or discard"), take the alternative
  only if the fix turns out not to be worth it, and log why.
- **Accessibility** (Decision: q-a11y, "Parity for consent, warning and status text"): `fix code` and `fix spec+code`
  items expose the consent, warning or status text to assistive technology (the SPEC.md principle itself is added once,
  by `antigravity-trust-warning-omitted-from-accessibility.md` in `triage-hooks-trust-nov24`; no other plan adds a copy,
  and a fix here that lands first cites the principle as decided on the 2026-11-24 triage page); `other` items are fixed
  only if the fix is about 20 lines or less, otherwise no change.
- **Polish: fix if cheap** (Decision: q-polish, "Fix the cheap ones, remove the rest"): fix it if the fix is about 30
  lines or less including a test; otherwise make no change. Either way the PR removes the item, and its Execution field
  says which way it went.
- **Test gap: fix if cheap** (Decision: q-tests, "Re-apply the Nov 21 rule"): fix the test only if the fix is a few
  lines (about 20 at most) and the test guards a user-facing behavior; otherwise make no change. Either way the PR
  removes the item, and its Execution field says which way it went. The "Recommendation for reference" in the criteria
  is advice for that judgment, not a decision.

### Acceptance criteria

1. Every outcome listed under Outline is executed exactly once, in one PR of this stack, as its ledger entry's Decision
   and Completion criteria say, retargeted only by the Decisions below. Each ends one of four ways: fixed as decided;
   judged no change where its criteria allow it; discarded under the unattended fallback; or deferred through the
   complexity gate.
2. A code fix makes the code match the rule its Assessment quotes, or builds what its Decision picked, with a test where
   practical. A fix that changes behavior SPEC.md or SPEC_impl.md describes updates them in the same PR.
3. Each outcome's Execution field, as moved to `TRIAGE_OUTCOMES_ARCHIVE.md`, names its PR (change ID, bookmark and link)
   and, where its criteria leave a choice or it was discarded or deferred, which way it went and why.
4. Each PR removes exactly its outcomes' feedback files and `review_feedback_queue/INDEX.md` lines, moves exactly their
   entries from `TRIAGE_OUTCOMES.md` to the end of `TRIAGE_OUTCOMES_ARCHIVE.md`, and lists them in its description. On
   the stack's tip none of this plan's outcomes remains in `TRIAGE_OUTCOMES.md` or the queue.
5. Each outcome deferred through the complexity gate has its TODO.md entry, in the `## Plan follow-ups` bucket, in the
   PR that removes it from the queue.
6. No PR exceeds about 300 lines of diff (not counting the ledger, the queue removals, changelog fragments or TODO
   entries), unless one outcome's fix needs it on its own.
7. Every PR that changes what the desktop app draws has passed its visual review and carries its re-captured, published
   golden pictures.

## Decisions already made

Settled with the maintainer at triage (each outcome's Decision) and on the 2026-11-24 planning page:

- **Ledger decisions are binding:** the triage-page decisions recorded in each outcome's ledger entry are executed as
  they stand in `TRIAGE_OUTCOMES.md`; this plan does not re-litigate them. The batching and complexity gate below change
  how they are executed, not what was decided.
- **Batching:** execute the outcomes as one PR per coherent group (outcomes touching the same module or of the same
  kind), at most about 300 lines of diff per PR. This deliberately overrides the one-PR-per-outcome rule in
  `review_feedback_queue/.agents/execute-outcomes.md` for these plans. Each PR carries the queue removals and ledger
  graduations for exactly the outcomes it executes and lists them in its description.
- **Complexity gate:** an outcome whose fix turns out not to be straightforward, or to exceed its recorded size gate
  (about 100 lines of diff, not counting the ledger, the queue removal or a changelog fragment), or that turns very
  complicated where its criteria carry the maintainer's own complexity gate, becomes a TODO.md entry in the
  `## Plan follow-ups` bucket describing the work, with enough of the finding's detail (what is wrong, where, the rule
  it breaks, the decided fix, and what made it complicated and why) that a later agent needs nothing else. The PR that
  would have carried the fix adds the entry, removes the feedback file and its index line, and graduates the ledger
  entry with an Execution field saying it was deferred to that entry; the report says so too, under "things you should
  know". The plan keeps going; this is not a block. It replaces the ledger criteria's "over that, execution blocks and
  brings the item back to the maintainer", "stop and push back to the maintainer" and "report that back to the
  maintainer" clauses. Adding these entries is the maintainer's instruction, not an entry on your own initiative.
- **Review gate:** one fresh-context `gpt-6.2-sol` at `high` effort agent per PR, with the general review charter quoted
  under Review gate below. Its findings are inputs, not orders.
- **Visual review:** any PR that changes what the desktop app draws additionally gets a fresh-context `opus` agent at
  `high` effort reviewing headless renders of the affected views in both appearances (the GUI golden scenes) against the
  outcome's recorded decision, looped until it reports no mismatch (see Visual review below).
- **Release note:** this plan's `## Release note` section is the maintainer's answer for the release's highlights. PRs
  still add changelog fragments when their type requires one (root `AGENTS.md`, "Changelog fragments").
- **Unattended fallback:** decided outcomes that are no longer relevant are discarded rather than brought back to the
  maintainer (maintainer, 2026-11-21, reaffirmed on the 2026-11-24 planning page); the wording is under Unattended
  fallback below.
- **No-workhorse mode:** the plans default (`plans/.agents/planning.md`), stated on the planning page.

## Requirement sources

**The maintainer's request and decisions:** the recorded triage outcomes (each ledger entry's Decision and Completion
criteria, including the maintainer's comments they quote), the maintainer's notes on execution quoted above, and the
decisions above.

**Binding repository constraints:**

- `review_feedback_queue/AGENTS.md` and `review_feedback_queue/.agents/execute-outcomes.md` (read both before changing
  anything under `review_feedback_queue/` or the ledgers); the batching decision above is the only departure from the
  latter, apart from the unattended fallback.
- The SPEC.md and SPEC_impl.md sections each outcome's Assessment cites.
- `.agents/test-authoring.md` for every test added or changed.
- Root `AGENTS.md`, "Code rules", "Conventional Commits", "Changelog fragments", "Tests and validation", "TODO.md and
  FLAKES.md" (for TODO entries), "Frozen material", and its "Instructions by path" for every path you touch.

**Planner proposals:**

- The grouping of outcomes into PRs under Outline, by kind. You may regroup when a group turns out incoherent (a PR that
  would exceed about 300 lines, or outcomes a reviewer cannot judge together), or to put outcomes that touch the same
  module together, logging why as a DECISION; every outcome stays in exactly one PR of this plan.
- Keep each fix to what its outcome needs; no refactors beyond it.

## Outline

1. `plan/triage-sessions-ui-nov24/01-notices`: outcomes in `notices`. Executes:

   - `trailing-duplicate-interrupted-upload-notices-silently-dropped.md` (fix as decided): While an interrupted-upload
     banner remains undismissed, a later interrupted upload of the same file name (including
   - `trailing-interrupted-upload-empty-title-and-batch-title-drop.md` (fix as decided): An interrupted upload in a
     session with an empty title renders 'to was', and batched interrupted uploads discard a

   May change what the app draws: judge from the diff whether the visual review applies.

2. `plan/triage-sessions-ui-nov24/02-whats-new`: outcomes in `whats-new`. Executes:

   - `upgrade-highlights-can-open-after-sign-in.md` (approved as listed): Upgrade highlights
   - `upgrade-highlights-opt-out-refusal-is-hidden.md` (approved as listed): A refused Don’t

   May change what the app draws: judge from the diff whether the visual review applies.

3. `plan/triage-sessions-ui-nov24/03-session-list`: outcomes in `session-list`. Executes:

   - `sidebar-retry-warning-missing-accessible-name.md` (accessibility): A retrying
   - `trailing-muted-waiting-dots-bypass-ringless-status-tooltip-path.md` (accessibility): On a muted session waiting on
     a connected host, hovering the waiting dot shows the row summary. If the summary fits

   May change what the app draws: judge from the diff whether the visual review applies.

4. `plan/triage-sessions-ui-nov24/04-other-fixes`: remaining fixes, one or two per module. Executes:

   - `trailing-interrupted-uploads-invisible-on-card-or-retired-session.md` (fix as decided): If an upload is cut off
     when the open session switches to a recovery, retired, or post-reset unavailable card,
   - `menu-shortcuts-omitted-from-native-accessibility.md` (accessibility): The new
   - `trailing-reminder-frame-cfg-test-branch-in-render-path.md` (fix, clearly settled): The pending-update reminder
     screenshot test relies on a test-only compile branch inside the rendering path,
   - `trailing-bell-open-announcements-silent-on-read-failure.md` (fix, clearly settled): Pressing Open Announcements in
     the notification bell does nothing visible when the helm read fails: no dialog and no

   May change what the app draws: judge from the diff whether the visual review applies.

5. `plan/triage-sessions-ui-nov24/05-cheap-fixes`: conditional items (fixed only if cheap, otherwise no change).
   Executes:

   - `trailing-all-phase-fixture-does-not-compare-managers-phase-strings.md` (test gap: fix if cheap): The strengthened
     fixture catches client/wire phase drift. A manager-only rename for a phase can still pass this
   - `trailing-status-mark-tests-lost-their-per-target-geometry-check.md` (test gap: fix if cheap): The row status tests
     verify that each mark's help appears with a notch and no ring. They can pass if placement uses
   - `trailing-maximized-relaunch-contract-has-no-first-frame-or-subsequent.md` (test gap: fix if cheap): The checked
     cases could pass if a maximized Mac relaunch still flashed the ordinary frame or if the next Zoom
   - `trailing-overflowing-banner-stack-resets-to-oldest-after-reconnect.md` (polish: fix if cheap): When an overflowing
     banner stack reconnects or takes back control and its newest notice was already drawn before
   - `trailing-light-theme-primary-and-danger-buttons-identical-hover-held.md` (polish: fix if cheap): In Light
     appearance, pressing and holding a primary or destructive button produces no visual change from hovering
   - `trailing-ticker-leaks-dead-subscribers-under-reduced-motion.md` (polish: fix if cheap): When the user keeps the
     operating system's Reduce Motion setting turned on, Farhelm's GUI animation list accumulates
   - `trailing-toolbar-650px-height-test-unguarded-else-branch.md` (test gap: fix if cheap): The layout test meant to
     guard the condensed environment button's 23-pixel height in a 650-pixel session pane does
   - `trailing-bell-stale-reply-guards-untested.md` (test gap: fix if cheap): No test covers the

   May change what the app draws: judge from the diff whether the visual review applies.

## Fallbacks

- An outcome whose fix needs a design choice that the spec and its ledger entry do not settle, or much more than its
  size gate, takes the complexity gate under Decisions rather than blocking.
- An outcome whose finding is already fixed on main, or whose code no longer exists, follows the unattended fallback
  below.
- Decisions to log: every regrouping, every overlap with another plan's area, every fix-or-no-change judgment and its
  reason, every use of the complexity gate, every outcome discarded under the unattended fallback, every review finding
  declined with its reason, and any criterion found already met or moot.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-triage-sessions-ui-nov24-log.md` in the parent directory of the checkout you run in, derived as that
section says, and the watchdog's status file is `farhelm-plan-triage-sessions-ui-nov24-watchdog.json` beside it. Resolve
both to absolute paths before you start; this file does not create either. Scratch files go in the agent scratch
directory that `AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation". Choose the narrowest checks that exercise what each PR changes. Apply `.agents/test-authoring.md` to every
test you add or change, and run `python -B scripts/check-test-sleeps.py` as `docs/test-sleep-check.md` prepares it when
Rust tests change. A PR that changes only comments, docs or spec prose with no doctest or generation role is
documentation-only: no runtime tests, only the formatters and checks that inspect the changed files (`dprint check`, and
`python3 scripts/check-agents-md.py` when a Markdown heading or file it cites changes).

### Resume protocol

Your first action, after confirming that the checkout you run in is a checkout of `scode/farhelm` (the nearest Git
worktree or jj workspace containing your working directory, one of whose remotes points at that GitHub repository; if
not, stop, write nothing, and say so), is to invoke the `agent-resumeable` skill with the log file's absolute path. If
the log exists, read it and resume where the previous session left off, cross-checking it against reality (the jj graph,
bookmarks and open PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier
executor worked on, or that came back from review, also gets the resume check in `plans/.agents/executing.md`,
"Executing one plan" (step 7) before any work.

### Execution mode: no-workhorse

Never send a unit of your own decomposition to a weaker model or a lower effort than your own. Never invoke, load or
follow `scode-galaxy-brain` or `scode-model-routing` during this plan. No-workhorse is the mode
`plans/.agents/planning.md` sets for plans, and the maintainer saw it stated while planning, so it is an explicit
choice: if galaxy-brain is already active when you start, this file is the instruction to stop it.

Sub-agents at your own model and effort are fine, and worth considering when they are likely to finish the work sooner
without costing quality (independent outcomes in one PR investigated in parallel, say). Concurrent agents that might
otherwise conflict can work in separate jj workspaces whose results you merge back; you still own the stack and every
PR. Launch a sub-agent natively, with model and effort set explicitly where the mechanism allows; when the model you
need is not available as a native sub-agent type (a reviewer, for instance), `$scode-harness-shellout` supplies the
launch mechanics.

The review gate and the visual review keep the model and effort this file demands; the scope reassessment runs at your
own model and effort. The sub-agents `plans/AGENTS.md`, "Sub-agents this file requires", requires of every plan (the
resume check, and the cold reads of a blocked question and of the report) are exempt from the rules above and run as
that file says: native, read-only, at your own model.

### Resource watchdog

Right after the resume protocol, and before the first sub-agent, build or test run, start the `scode-build-goal` skill's
watchdog and keep it running for the whole plan. It is the Deno script `scripts/watchdog` in that skill's directory;
resolve its path with `realpath` so it is the same file whichever harness runs you, and read its module docs. Its status
file is the one named under "Files outside the repository" above. Watch, each as a `--path` of `sample`: the root of the
checkout you run in, the home directory, `/tmp`, the directory holding the status file, your scratch directory, and any
jj workspace or build directory you later create on another filesystem (restart the sampler with it added). Sample every
60 seconds, at the script's default alert levels.

- In Claude Code, run `watchdog observe --status <file>` under the Monitor tool, then
  `watchdog sample --status <file> --path <dir>...` under a second Monitor, both at the maximum timeout, then run
  `watchdog check --status <file>`. Every line either prints is a notification. Whenever either is re-armed, stop the
  other and arm both again, observer first, then `check`. An expiry notice is routine; a monitor exit is a monitoring
  failure.
- In Codex, arm `watchdog observe --status <file> --exit-on-event` and then the sampler as two separate runs through the
  `skillette-codex-wake-on-exit` mechanism (load the `skillette` skill and follow that entry), then run `check`. On the
  observer's callback, acknowledge it and arm a fresh observer; after a `STALE`, also stop the hung sampler through its
  handle and arm a new one; then run `check` and act.
- Before starting the real monitors in each session, prove the mechanism on throwaway status files in your scratch
  directory with `--interval 5`: a sampler run with `--synthetic-alert` delivers an `ALERT`; a sampler killed through
  the `pid` in its status file is reported; a sampler run with `--stall-after 2`, watched by its own observer armed
  first, produces `STALE` within two sample intervals. Stop the throwaway runs afterwards. If the mechanism is
  unavailable or fails this, say so in the log, run the sampler as a plain background process, and rely on `check`.
- A shortage (an `ALERT` or `WORSE` line, or `check` reporting one) is an instruction to act: stop launching new
  sub-agents and work, remove jj workspaces, build output and scratch you own that are no longer needed, wait for or
  stop the job most likely responsible, and resume launches only once `check` passes. A monitoring failure (`STALE`,
  `ERROR`, a monitor exit, `check` failing on heartbeat age) pauses new launches until monitoring is restored; stop a
  hung sampler before starting another so two never write one status file.
- Run `watchdog check --status <file>` before every new build, test run, sub-agent or review launch, and at natural
  boundaries. Record the monitor handles, status file and watched paths in the log and carry them in every handoff; a
  resumed session whose status file is still fresh stops the earlier session's monitors (through the recorded handles,
  or the `pid` in the status file) before starting its own. Stop the monitors when the plan closes.

### PR discipline

- One reviewable commit, stable bookmark and draft PR per outline step, in a single linear stack, using the `jjstack`
  skill. The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/.agents/executing.md`, "Executing one plan" (step 6); never another plan's PRs. Bookmarks:
  `plan/triage-sessions-ui-nov24/01-notices`, `plan/triage-sessions-ui-nov24/02-whats-new`,
  `plan/triage-sessions-ui-nov24/03-session-list`, `plan/triage-sessions-ui-nov24/04-other-fixes`,
  `plan/triage-sessions-ui-nov24/05-cheap-fixes`.
- Keep each PR to its group and about 300 lines of diff (not counting the ledger, the queue removals, changelog
  fragments or TODO entries). Avoid churn: code added in one PR and deleted in a later one means the stack should have
  been shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and
  that applies to PRs an earlier run built too.
- Each PR executes its outcomes per `review_feedback_queue/.agents/execute-outcomes.md`, "Execute triage outcomes",
  except that it carries several outcomes: for each one it removes the feedback file and its
  `review_feedback_queue/INDEX.md` line (or narrows the file, for a partial outcome), and moves the entry from
  `TRIAGE_OUTCOMES.md` to the end of `TRIAGE_OUTCOMES_ARCHIVE.md` with its Execution field replaced by the change ID,
  bookmark and draft PR link, plus which way it went where the criteria leave a choice (fixed; no change, and why;
  discarded under the unattended fallback, and why; or deferred to a named TODO.md entry). Record the change ID and
  bookmark before creating the PR, then add the link and update the PR. Each entry's Execution reads
  ``planned in `plans/queue/triage-sessions-ui-nov24.md` `` until then. The PR description lists the outcomes it
  executes.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`): a PR of comment and doc corrections is `docs`, test-only changes are `test`, queue bookkeeping is
  `chore`, and a PR that fixes something users see is `fix`. Add a changelog fragment in the same commit when the type
  calls for one (root `AGENTS.md`, "Changelog fragments"): one fragment may cover a PR's user-visible fixes when they
  read as one entry, otherwise one per fix, and a `kind: none` fragment with its reason when nothing user-facing
  changed. This plan's `## Release note` section is the maintainer's answer for the release's highlights, not an
  instruction to you (`plans/.agents/executing.md`, "Executing one plan", step 8).
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides. Every other plan of this batch edits them
  too, so expect such conflicts. A conflict anywhere else (SPEC.md included) is a careful rebase
  (`.agents/careful-rebase.md`).
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The maintainer
demands exactly this reviewer, and no review swarm: a single fresh-context agent on `gpt-6.2-sol` at `high` effort,
launched natively or through `$scode-harness-shellout` when your harness cannot serve that model natively. If it cannot
be reached when a review is due and is still unreachable after a reasonable retry, block the plan per
`plans/.agents/executing.md`, "Executing one plan" (step 10), after pushing what is consistent. Never substitute another
reviewer.

The prompt carries this full charter, because the reviewer has nothing else:

> Review the changes in this PR adversarially for general correctness (bugs, unhandled cases, broken invariants, wrong
> behavior against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and
> the goal, and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the
> named file, most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing
> and touch no VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range of this one PR, a
findings file in the scratch directory, and the acceptance criteria: this file's goal, acceptance criteria and
decisions, and for each outcome the PR executes, its full ledger entry and its feedback file. When a PR changes tests or
their fixtures, include the full text of `.agents/test-authoring.md` verbatim in the prompt, as root `AGENTS.md`
requires.

The maintainer's standing rule for this reviewer (2026-11-17): its findings are inputs, not orders. Take a correctness
finding (a bug, a missed case, a wrong claim) after checking it yourself. A finding that would add code, a new
mechanism, a new test layer or another round of hardening is declined by default unless you judge it plainly necessary;
never let a review grow a PR. Log each finding you decline as a DECISION with its reason. Do not write a launch command
here or in the log; the harness-shellout skill owns launch mechanics.

### Visual review

Mandatory, in addition to the review gate, for every PR that changes what the desktop app draws (judge each PR from its
diff; the outline marks the PRs likely to): the maintainer demands a fresh-context agent on `opus` at `high` effort,
launched natively or through `$scode-harness-shellout`. Never substitute another model, and never skip it because the
review gate passed. If it cannot be reached after a reasonable retry, block per `plans/.agents/executing.md`, "Executing
one plan" (step 10).

- **What it looks at.** Render the affected views headless, light and dark, with `scripts/check-gui-goldens.py --update`
  (target the affected scenes with `--scene` while iterating, then run the full update before publishing, as
  `.agents/validation.md` describes), which writes the captures to `target/gui-goldens/captures`. Give the reviewer the
  paths of every new or changed picture, and of the current reference pictures for the same scenes where they exist.
  When no golden scene shows a view the PR changes, add one, light and dark, in the same PR.
- **What it judges against.** Each outcome's recorded decision: its ledger entry (Assessment, Decision, Completion
  criteria), the SPEC.md or SPEC_impl.md text the entry cites, and this file's acceptance criteria. Name in the prompt
  which outcomes the PR executes and which views each changes; parts of a view the PR does not touch are not findings.
- **Its charter**, carried in full in the prompt:

  > Compare the rendered screenshots of this PR's views, in both appearances, with what each outcome's recorded decision
  > and the cited spec text say the app should show. Judge whether each picture shows the decided behavior: the wording,
  > which controls and states appear, and layout and hierarchy where the decision speaks to them, and that nothing the
  > PR touched is cut off, overlapping, illegible in either appearance, or visibly broken. Pixel identity with older
  > reference pictures is not the bar, and parts of a view the PR does not touch are not findings. Report findings to
  > the named file, most severe first, each naming the picture, what differs from the decision, and what would bring it
  > in line, and end with one line: NO MISMATCH or MISMATCH. Edit nothing and touch no VCS state.

- **Looping.** Fix what it finds that is a real mismatch, re-capture, and run a fresh visual review on the new pictures,
  until it reports NO MISMATCH. A finding you judge wrong is declined with a DECISION in the log and the picture and
  reason named in the report; a review that reports MISMATCH only on declined findings counts as passed. If the same
  mismatch survives three rounds, run the scope reassessment below on it before another round.
- **Publishing.** Publish the accepted captures with `python3 scripts/gui-goldens.py publish` and commit the manifest
  change with the PR, before any rebase, as `.agents/validation.md` describes. Look at every picture its report names as
  changed or added before committing.

### Scope reassessment

Run a fresh-context review at your own model and effort:

- before implementing a substantial departure from the outline above (the plan-specific examples are under Fallbacks
  above; they are examples, not a blacklist), including a new mechanism that does the job of an existing one or a
  generalization the outline did not plan;
- whenever the same component has needed repeated corrective review rounds.

Supply this file's goal and decisions, the outline, the current diff, and the proposed departure: what changed, why it
is necessary, and which simpler alternative was ruled out. Use this charter:

> Identify where this plan does more than the user needs, and where it duplicates a mechanism the repository already
> has. For each substantial new mechanism, explain which requirement makes it necessary and whether an existing facility
> or narrower supported behavior would suffice. Where the plan builds something that does substantially the same job as
> an existing mechanism, say whether that mechanism could be reused as it stands or generalized to serve both uses, or
> whether keeping them separate is better because sharing would blur two different contracts, and what generalizing
> would cost; never propose generalizing beyond the uses the plan actually has. Weigh the two against each other: a plan
> is not simpler for copying a mechanism, and not better for generalizing one nobody else needs. Propose the smallest
> coherent design that avoids needless duplication. Challenge planner-authored requirements; identify any simplification
> that would change an explicit user requirement or repository constraint, and mark any reuse-versus-duplication choice
> that is a close judgment call rather than clear-cut. Report concrete findings and alternatives, or say no unnecessary
> complexity or duplication was found. Do not manufacture objections, add safeguards to justify an unnecessary
> subsystem, or add abstraction to justify a generalization. Write findings to the supplied private file; change no
> product files or VCS state.

A close reuse-versus-duplication call that stays within the agreed scope is yours to decide after the reassessment, and
to log as a DECISION. Reuse an existing review checkpoint when it can answer the question; do not schedule periodic
reviews. A sub-agent gets a bounded unit within the outline; an unapproved new mechanism is outside its task, and one
discovered mid-unit is left unimplemented and reported back, with checks and incomplete work stated honestly. Apply the
same decision boundary as Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular those named under Fallbacks above, and every review finding you decided not to
follow. The maintainer will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the maintainer's decisions and the repository constraints still
hold. Wording in this file that turns out inaccurate (a line number, a file name, a test name) is recorded in the
working log with the true value; never edit this plan file. If current code already does what an outcome asks as a
whole, or the code it targets no longer exists, execute that outcome as a discard (queue removal and ledger bookkeeping,
with the reason in its Execution field), log it as a DECISION, and say so in the report. This departs from
`review_feedback_queue/.agents/execute-outcomes.md`, which returns an invalidated decision to the user: the maintainer
asked on 2026-11-21 that decided outcomes no longer clearly relevant be discarded rather than brought back. An
acceptance criterion that is already true by design (one that only says what must be kept) is not a reason to discard.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the
maintainer's decision; a review finding or a log entry is not authorization. If no fallback covers it, record the
concrete tradeoff and continue with independent work. Block per `plans/.agents/executing.md`, "Executing one plan"
(step 10) only for what cannot proceed without the maintainer, after pushing every PR that can be finished; an outcome
that would need such a decision takes the complexity gate instead whenever that keeps the rest moving.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists, one per outline step in the order the outline gives (or
per a regrouping you logged), together executing every outcome listed under Outline exactly once and meeting the
acceptance criteria above, each having passed its review gate (and, for a PR that changes what the app draws, its visual
review) and each carrying its outcomes' queue removals and ledger moves. Open, not merged. If a `## Decisions` section
exists, its latest entry must also be satisfied. Then close the plan per `plans/.agents/executing.md`, "Executing one
plan" (steps 11 and 12): deliver its report through the queue script, write a closing entry in its log, and stop the
watchdog. The report maps every outcome to its PR and says how each ended (fixed, no change, discarded, or deferred to a
TODO entry), and names every overlap with another plan's area. Never edit `plans/` yourself.

## Release note

No release note needed.
