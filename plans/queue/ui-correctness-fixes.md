# Execute thirteen triage outcomes: small bugs in the web and desktop UI

Written against main at 9046a0db on 2026-10-10. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan runs after `gui-text-safety.md`: that plan also changes the template shape check (`check_template_shape` in
`crates/farhelm-proto/src/launcher.rs`), which `template-dot-name.md` changes here, so this plan starts once that work
is on main.

## The goal

Carry out thirteen triage outcomes recorded in root `TRIAGE_OUTCOMES.md` in one draft PR. Their headings, which are also
the feedback file names under `review_feedback_queue/`, all with outcome `fix code`:

- `grok-trust.md`
- `template-dot-name.md`
- `non-ascii-template-names-lose-exact-match-priority.md`
- `ime-confirmation-save-template-prematurely.md`
- `font-focus.md`
- `selecting-osc-hyperlink-open-it-unintentionally.md`
- `older-listing-replace-newly-opened-session-another.md`
- `failed-notification-read-mark-suppresses-subsequent-retries.md`
- `update-popup-stops-following-current-step-opening.md`
- `earlier-setup-uninstall-error-hides-later-update.md`
- `rename-refusal-text-bypasses-peer-text-rendering.md`
- `restart-refusal-text-bypasses-peer-text-rendering.md`
- `rename-original-title-display-conceals-title-characters-original-title-original-title-display.md`

All thirteen are bugs a person using the web or desktop UI can hit, each fixable in one place, mostly in
`crates/farhelm-ui` and one in `crates/farhelm-proto`. Several match patterns the codebase already uses elsewhere (a
composition check, a selection guard, a listing-read fence, peer-text rendering), and the fix is to apply the existing
pattern where it is missing.

Each ledger entry (Assessment, Decision, Completion criteria) is the authoritative statement of what the PR must do for
that outcome. Read every entry and feedback file before starting. Where this file and a ledger entry seem to disagree,
the ledger entry wins and the disagreement is a DECISION to log (or a gate trip, per Complexity gate).

Acceptance criteria:

- One draft PR carries out every outcome that did not trip its complexity gate, each meeting its ledger entry's
  completion criteria.
- Each fix has a regression test where existing tests or fixtures can observe it. Where a test would need new test
  machinery (a new seam, new browser setup, captured screens), the fix ships without one and the log and report say why:
  a test that needs new machinery is skipped, never the fix.
- The PR removes each carried-out outcome's feedback file and index line, updates every outcome's `TRIAGE_OUTCOMES.md`
  Execution field, adds a changelog fragment where root `AGENTS.md` requires one, and passed the review gate.
- The PR is not marked ready, and nothing is merged.

## Requirement sources

**The user's triage decisions (2026-10-10):** rather than triage the undecided review findings one by one, the user
asked for a screen of the whole queue against these criteria, in their words:

> - it is CLEARLY a bug. no judgement needed to answer whether it's something that should ideally be fixed.
> - the fix is reasonably straight forward and does not cause complexity or scope creep.
> - it is not mooted because code has changed.

The screen found 96 such findings, and the user chose to fix all of them and to "schedule them for execution in the
planning system with a complexity gate - if it turns more complicated than expected, skip and keep it unfixed for
triage", grouped "so we end up with roughly 10 plans", similar or related findings together. Each outcome's
`TRIAGE_OUTCOMES.md` entry records this and is the authoritative statement of what was decided. The screen was done by
reading code on main at `83516c6c`, not by running anything, so each assessment is a claim to confirm before fixing.

**The user's plan-time decisions (2026-10-10):** one PR per plan, overriding root `AGENTS.md`'s one-PR-per-outcome rule
for these outcomes only; each outcome has a complexity gate, and an outcome that trips it is dropped from the PR while
the rest ships; review gate gpt-6.1-sol at high effort; no-workhorse mode, which `plans/AGENTS.md` requires.

**Planner proposals** are the fixes and placements in the outline below, taken from each ledger entry's completion
criteria and checked against main by a fresh-context planning review.

**Binding repository constraints:** root `AGENTS.md` (Conventional Commits; Finishing work; Execute triage outcomes, as
overridden above; Releases and the changelog; Agent scratch space; Sharing the machine with other agents; the rule
against tests that modify the test process's own environment variables; SPEC.md and SPEC_impl.md are authoritative and
changed only as decided), the user's global rules to write documentation and comment prose with the `scode-voice` skill
and to give new and changed code, tests included, docstrings and comments that explain the why, `plans/AGENTS.md`
(Executing one plan), `review_feedback_queue/AGENTS.md` (Lifecycle), and `.agents/test-authoring.md` for test changes.

## Outline

Line numbers drift; find the code by name. The fix for each outcome, from its ledger entry (where the code was at
screening time in parentheses):

- `grok-trust.md`: Clear the workspace-trust choice in the Grok early-return branch as well.
  (`crates/farhelm-proto/src/launcher.rs`)
- `template-dot-name.md`: Refuse dot-only names in the template shape check with a clear message.
  (`crates/farhelm-proto/src/launcher.rs`)
- `non-ascii-template-names-lose-exact-match-priority.md`: Use the same Unicode lowercasing for the exact-match check as
  the filter. (`crates/farhelm-ui/src/launch_composer.rs`)
- `ime-confirmation-save-template-prematurely.md`: Add the same composition check before saving on Enter.
  (`crates/farhelm-ui/src/list/save_template.rs`)
- `font-focus.md`: Hand focus back to the terminal even when the size does not change; keep the font update and refit
  only for a real change. (`crates/farhelm-ui/assets/terminal.js`)
- `selecting-osc-hyperlink-open-it-unintentionally.md`: Add the same non-empty-selection guard to the OSC 8 handler's
  `activate`. (`crates/farhelm-ui/assets/terminal.js`)
- `older-listing-replace-newly-opened-session-another.md`: Fence earlier listing reads on the create and replace success
  paths, as delete already does. (`crates/farhelm-ui/src/list/view.rs`)
- `failed-notification-read-mark-suppresses-subsequent-retries.md`: On failure, roll back the cached mark for that
  session if it still holds the mark that failed, so a newer successful mark is not undone.
  (`crates/farhelm-ui/src/list/view.rs`)
- `update-popup-stops-following-current-step-opening.md`: Make the scroll effect also re-run when the current step
  changes. (`crates/farhelm-ui/src/hosts.rs`)
- `earlier-setup-uninstall-error-hides-later-update.md`: When an Update starts, also clear the setup/uninstall error and
  warning. (`crates/farhelm-ui/src/provisioning.rs`)
- `rename-refusal-text-bypasses-peer-text-rendering.md`: Render the error through the existing peer-text rendering, as
  the row error line does. (`crates/farhelm-ui/src/rename.rs`)
- `restart-refusal-text-bypasses-peer-text-rendering.md`: Render the error through the existing peer-text rendering.
  (`crates/farhelm-ui/src/restart_with.rs`)
- `rename-original-title-display-conceals-title-characters-original-title-original-title-display.md`: Show the current
  title through the existing peer-title rendering; the stored title is not touched. (`crates/farhelm-ui/src/rename.rs`)

The OSC 8 link handler in `crates/farhelm-ui/assets/terminal.js` changed after these findings were written: it now also
downloads file targets. The missing selection guard applies to every activation, web or file. The comment on the
plain-link handler saying OSC 8 gestures are untouched becomes false; rewrite it.

The existing title rendering to reuse for the rename dialog is the `PeerTitle` component in
`crates/farhelm-ui/src/list/row.rs`; widening its visibility to the crate is in scope. The update popup's scroll effect
re-runs on a step change only once the current step is a reactive dependency of it.

`ui-interaction-fixes.md` also edits `terminal.js`, in different functions; read what has landed from it first.

First confirm each assessment against current main: if the problem is already gone, that is not a gate trip; record the
outcome as `discard` with "already fixed" and the fixing commit, per root `AGENTS.md`, and remove its queue item.

Commit type and changelog: `fix:` with a changelog fragment (`kind: fixed`) written for someone running Farhelm, naming
the user-visible fixes in a few plain sentences. Validate a fragment with `python3 releasing/check-changelog.py format`.

The complexity gate for each outcome is the fix named above. A new mechanism, plumbing across components, a change well
outside the named files, or a product or design decision is a gate trip for that outcome. A regression test that would
need new machinery is skipped and logged, never a reason to drop the fix.

### Validation

Follow root `AGENTS.md` "Finishing work": `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo check -p farhelm-ui --features desktop` if the desktop dependencies are present, focused nextest selections for
the touched `farhelm-proto` and `farhelm-ui` unit tests through `scripts/record-test-run.py`, and
`cd crates/farhelm-ui/js-tests && node --test` when JS assets change. Add a regression test for a fix where existing
tests or fixtures can observe it, at the cheapest level (unit, JS harness, or an existing Playwright spec); where that
would need new test machinery, skip the test and log why. Where a changed behaviour is covered by existing Playwright
specs, run those specs through the recorder on Chromium and WebKit, per root `AGENTS.md` (Browser end-to-end
validation); do not run the full browser suite. Run `python -B scripts/check-test-sleeps.py` when tests change, and
apply `.agents/test-authoring.md`.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-ui-correctness-fixes-log.md` in the parent directory of the checkout you run in, derived as that section
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
  `plan/ui-correctness-fixes/01-ui-correctness-fixes`.
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
verbatim, as root `AGENTS.md` requires. Also ask the reviewer to check that each fix reuses the pattern the codebase
already has for the same problem elsewhere rather than a new one. Where you disagree with a finding, decide on the
merits and log the DECISION. Do not write a launch command here or in the log; the harness-shellout skill owns launch
mechanics.

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
alternatives considered: in particular how each assessment held up against current main, the Conventional Commit type
and changelog kind, every outcome dropped by its complexity gate or discarded as already fixed and why, every
strengthened test's mutation check or the reason there is none, and every review finding you decided not to follow. The
user will ask for these later.

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
