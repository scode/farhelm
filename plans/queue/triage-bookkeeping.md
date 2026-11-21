# Triage outcomes: discards and the duplicated-code TODO entry

Written against main at e91ae9c6a on 2026-11-21. This is a goal file for an unattended run, executed through the
planning system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it;
everything you need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over
anything this file says.

## The goal

Execute the 62 triage outcomes listed under Outline below, as a short stack of batched PRs. Each outcome is recorded in
`TRIAGE_OUTCOMES.md` under the name of its feedback file in `review_feedback_queue/`, and was decided on 2026-11-21 on
the maintainer's fewest-questions triage page (against main ca62397de). The ledger entry's Assessment and the feedback
file hold the finding; its Decision and Completion criteria say what to do, and they bind except where this file's
Decisions retarget them.

### This plan's area, and the nine others

This plan changes no code and no spec. It touches only `review_feedback_queue/` (the feedback files and `INDEX.md`),
`TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md` and `TODO.md`.

This is one of ten plans that together execute every outcome triaged on 2026-11-21. The maintainer asked for them to be
split by area of code so they can run concurrently ("yes we definitely need to batch these in a much smaller set of
plans. please try to separate them by area of code so there is a good chance of them running concurrently."). Any of
them may run at the same time as this one. The other nine, and the areas this plan leaves to them:

- `triage-spec-text`: SPEC.md and SPEC_impl.md text, and user docs text, that records accepted current behavior.
- `triage-gui-sessions`: the desktop app's session list, sidebar, attention list, sign-in prompts, notices and session
  cards.
- `triage-gui-misc`: the rest of the desktop app: launcher, templates, Settings, Hosts panel, approval cards, terminal
  view and platform input.
- `triage-supervisor`: the supervisor's service (session lifecycle, Restart, Delete, tabs, status sampling, its store)
  and the e2e suite and its fixtures.
- `triage-terminal-host`: the terminal path: the terminal host crate, the terminal emulator, the supervisor's grid
  output and host terminal runtime, the helm's terminal relay, the vendored terminal crates, and test-state cleanup of
  terminal hosts.
- `triage-helm`: the helm's provider-neutral code (hosts, provisioning, automatic update, tunnels, sessions and Trash,
  notices, agent requests, credential delivery), the CLI, and the desktop app's startup and updater.
- `triage-podman`: the Podman provider: container builds, probes and removal, container hosts and their identity, the
  connection manager's container branch, and the real-container e2e module.
- `triage-tensorlake`: Tensorlake sandboxes: the helm's Tensorlake code, adding a sandbox host, sandbox session
  lifecycle, and the Tensorlake test runner.
- `triage-tooling`: maintainer tooling: scripts, release and brick-test tooling, deflake, docs-media and agent-screen
  capture, the test recorder, worker and summary, the plans queue and landing scripts.

Do not change code in those areas unless an outcome of this plan genuinely requires it. When one does, keep the edit to
what the outcome needs, log the overlap as a DECISION, and name it in the report. Known overlaps:

- Several discards (the Q1 races) rest on a spec sentence that `folder-suggestion-enter-can-submit-launcher.md` adds in
  `triage-gui-misc`. Executing the discards does not wait for that sentence: a discard only removes the item, and the
  ledger entry keeps the reasoning.
- Two duplicated-code findings note other plans that churn the same code
  (`launcher-repeats-choices-and-summary-formatting.md`: the `launcher-command-field.md` plan;
  `gui-screenshot-scenes-repeat-fixture-wiring.md`: the `goldens-fast-and-targeted.md` plan). Carry those notes into the
  TODO entry; they are for whoever picks the entry up.

### What each kind of outcome asks for

The kind is read from the outcome's ledger Decision; the outline tags each outcome with it.

- **Discard** (Outcome `discard`): remove the feedback file and its index line and graduate the ledger entry. No spec or
  code change.
- **Duplicated code** (Decision: Q13): the findings leave the queue and their detail moves into one Near term TODO.md
  entry, in the form under Decisions.

### Acceptance criteria

1. The 45 `discard` outcomes are executed in PR 01: each feedback file and its `review_feedback_queue/INDEX.md` line
   removed, each ledger entry moved from `TRIAGE_OUTCOMES.md` to the end of `TRIAGE_OUTCOMES_ARCHIVE.md` with its
   Execution field filled in, and no spec or code change.
2. PR 02 adds exactly one Near term TODO.md entry in the form under Decisions, retaining every one of the 17
   duplicated-code findings' locations, what is duplicated and the suggested shape, and executes those 17 outcomes the
   same way, each Execution field naming the entry.
3. On the stack's tip, `review_feedback_queue/INDEX.md` lists exactly the feedback files left in the directory, none of
   the 62 outcomes remains in `TRIAGE_OUTCOMES.md`, each is present once at the end of `TRIAGE_OUTCOMES_ARCHIVE.md`, and
   no other ledger entry changed.
4. Each PR's description lists the outcomes it executes.

## Decisions already made

Settled with the maintainer at triage (each outcome's Decision) and on the 2026-11-21 planning page:

- **Ledger decisions are binding:** the triage-page decisions recorded in each outcome's ledger entry are executed as
  they stand in `TRIAGE_OUTCOMES.md`; this plan does not re-litigate them. The batching and complexity gate below change
  how they are executed, not what was decided.
- **Batching:** execute the outcomes as one PR per coherent group (outcomes touching the same module or of the same
  kind, such as all comment corrections in one crate), at most about 300 lines of diff per PR, roughly 3 to 10 PRs per
  plan (the bookkeeping and spec plans may be one or two PRs each). This deliberately overrides the one-PR-per-outcome
  rule in `review_feedback_queue/.agents/execute-outcomes.md` for these plans. Each PR carries the queue removals and
  ledger graduations for exactly the outcomes it executes and lists them in its description.
- **Complexity gate:** an outcome whose fix turns out not to be straightforward, or to exceed its recorded size gate
  (about 100 lines of diff, not counting the ledger, the queue removal or a changelog fragment), becomes a Near term
  TODO.md entry describing the work, with enough of the finding's detail (what is wrong, where, the rule it breaks, the
  decided fix) that a later agent needs nothing else. The PR that would have carried the fix adds the entry, removes the
  feedback file and its index line, and graduates the ledger entry with an Execution field saying it was deferred to
  that entry; the report says so too. The plan keeps going; this is not a block. It replaces the ledger criteria's "over
  that, execution blocks and brings the item back to the maintainer" and "stop and bring it back to the maintainer"
  clauses. An outcome whose criteria set no size gate ("Medium; accepted") is held to the not-straightforward test only.
  Adding these entries is the maintainer's instruction, not an entry on your own initiative. (No outcome in this plan
  should need it.)
- **Review gate:** one fresh-context `gpt-6.2-sol` agent at `high` effort per PR, with the general review charter quoted
  under Review gate below. Its findings are inputs, not orders.
- **Release note:** this plan's `## Release note` section is exactly `No release note needed.` PRs still add changelog
  fragments when their type requires one (root `AGENTS.md`, "Changelog fragments").
- **Unattended fallback:** decided outcomes that are no longer relevant are discarded rather than brought back to the
  maintainer (maintainer, 2026-11-21); the wording is under Unattended fallback below.
- **No-workhorse mode:** the plans default (`plans/.agents/planning.md`), stated on the planning page.
- **The 17 duplicated-code findings** (their Decision is Q13): the maintainer wrote "add a near term todo item for this
  instead. the findings go out of the queue, but the detail must be retaine din a way that the agent looking at the todo
  item later knows what the findings were". The planner chose the form (2026-11-21, after reading TODO.md's header and
  its existing entries): ONE Near term TODO.md entry, a bold title and a one-paragraph statement of the work
  (consolidate the duplicated helpers the 2026-11-21 review found, reusing an existing shared module where one fits and
  never generalizing beyond the callers in hand), followed by a nested bullet list with one item per finding file. Each
  item names the finding file, the files and functions (or reviewed locations) that repeat each other, what is
  duplicated, and the suggested shared shape, condensed from the feedback file's Details, keeping every sub-finding of a
  file that has several. The entry ends with where the full text lives: the removed feedback files at the main commit
  the PR is based on, as `git show <commit>:review_feedback_queue/<file>` (name the commit id; it is already on main, so
  the citation stays valid). Keep it inline even though it is longer than most entries (TODO.md already holds entries of
  50 to 90 lines); do not put the detail in `lore/` (lore entries are decision records added only on explicit request)
  or invent a new detail file. Condense wording rather than drop a location or a suggested shape.
- **Discards**: each discard's PR removes its feedback file and INDEX line and graduates its ledger entry; nothing else
  changes. The unattended fallback below does not apply to a discard: it is already the removal.
- **Batching for this plan**: one PR for the 45 discards and one for the duplicated-code entry. The about-300-lines rule
  does not apply here (the diff is almost entirely removed feedback files and moved ledger entries). Split the discards
  into two PRs only if the reviewer cannot read one; log it.

## Requirement sources

**The maintainer's request and decisions:** the recorded triage outcomes (each ledger entry's Decision and Completion
criteria), the maintainer's note on execution quoted above, and the decisions above.

**Binding repository constraints:**

- `review_feedback_queue/AGENTS.md` and `review_feedback_queue/.agents/execute-outcomes.md` (read both before changing
  anything under `review_feedback_queue/` or the ledgers); the batching decision above is the only departure from the
  latter, apart from the unattended fallback.
- Root `AGENTS.md`, "Conventional Commits", "Changelog fragments", "Tests and validation", "TODO.md and FLAKES.md" (for
  TODO entries), and its "Instructions by path" for every path you touch.
- Root `AGENTS.md`, "TODO.md and FLAKES.md": adding the Near term entry is the maintainer's instruction recorded in the
  ledger, not an entry on your own initiative. In a stack, fetch and read main's `TODO.md` rather than rebasing mid-run,
  and keep both sides on a `TODO.md` conflict.

**Planner proposals:**

- The grouping of outcomes into PRs under Outline. You may regroup when a group turns out incoherent (a PR that would
  exceed about 300 lines, or outcomes a reviewer cannot judge together), logging why as a DECISION; every outcome stays
  in exactly one PR of this plan.

## Outline

1. `plan/triage-bookkeeping/01-discards`: execute the 45 `discard` outcomes (queue removal and ledger graduation only).
   Executes:

   - `shell-tab-label-omitted-row.md` (discard): The shell tab can still say Agent when the pane has no session row
   - `cleanup-retry-omits-recovery-step-times.md` (discard): Cleanup Retry omits recovery-step timing
   - `trailing-ui-retained-whole-environment-question-stale-unreachable-rationale.md` (discard): Retained
     whole-environment warning can misstate current reachability
   - `group-divider-can-change-shift-sweep-range.md` (discard): Sweeping across a group divider can change which
     sessions are marked
   - `mute-menu-expiry-preview-can-go-stale.md` (discard): A mute menu can display an earlier end than it saves
   - `shift-sweep-can-mark-through-sidebar-clip.md` (discard): A Shift sweep can mark sessions through the clipped edge
     of the sidebar
   - `session-sign-in-suppresses-connection-diagnosis.md` (discard): Host sign-in can hide the session’s connection
     diagnosis
   - `new-host-attention-cue-can-be-lost.md` (discard): A new attention cue can disappear behind an older one
   - `late-tab-restore-hides-upload-notice.md` (discard): An interrupted upload can stay unreported while a reopened
     session has no row
   - `retired-connection-leaves-suspend-progress-stuck.md` (discard): Retired connection leaves suspend progress stuck
   - `settings-search-key-hidden-by-open-editor.md` (discard): Settings search can miss the Tensorlake key field
   - `trailing-podman-environment-d-drops-explicit-empty-proxy.md` (discard): Empty proxy variables disappear after the
     container user manager restarts
   - `snapshot-cleanup-races-new-host-registration.md` (discard): Snapshot cleanup races new host registration
   - `package-name-trailing-minus-requests-removal.md` (discard): Package name trailing minus requests removal
   - `recipe-destination-refuses-absolute-dns-name.md` (discard): Recipe destination refuses absolute dns name
   - `apt-regex-fallback-can-accept-package-typo.md` (discard): Apt regex fallback can accept package typo
   - `trailing-shared-recipe-container-base-summary-raw-presentation.md` (discard): Recipe summaries render literal
     container-base formatting characters
   - `filtered-tab-reset-can-route-input-before-display.md` (discard): A tab reset can send a key to the primary shell
     before it appears
   - `checkout-use-default-presentation.md` (discard): Use default can clear a checkout folder that has yet to appear
   - `upload-path-pastes-into-restarted-agent.md` (discard): An upload can paste its path into a restarted agent
   - `wayland-drop-lost-before-file-list-arrives.md` (discard): A quick Wayland drop can disappear without a notice
   - `sidebar-sign-in-hides-launch-refusal.md` (discard): Sign in can silently refuse outside Hosts
   - `add-host-close-reads-live-sign-in.md` (discard): Add Host’s close action can change after it is drawn
   - `provisioning-waits-use-fleet-slots.md` (discard): Interactive maintenance can hold all provisioning slots
   - `tunnel-control-exit-can-open-new-ssh-connection.md` (discard): Tunnel control exit can open new ssh connection
   - `cancelled-ssh-capture-can-retain-local-readers.md` (discard): Cancelled ssh capture can retain local readers
   - `automatic-update-opt-out-can-roll-back.md` (discard): Automatic update opt out can roll back
   - `trailing-podman-place-typed-ssh-refusal-reasons-missing-final-ui-escaping.md` (discard): Typed SSH capacity
     reasons bypass final display escaping
   - `antigravity-historical-restart-can-target-child.md` (discard): The update can retain a subagent ID as the existing
     session's Restart target
   - `mac-uninstall-unresolved-loaded-plist.md` (discard): Missing loaded plist skips uninstall's service-path
     comparison
   - `shell-idle-plays-agent-turn-finished.md` (discard): A shell can play the Turn finished sound without an agent turn
   - `quick-switcher-enter-can-run-a-different-template.md` (discard): Quick Switcher can act on a template the user has
     not seen
   - `deferred-template-changes-unpresented-launch.md` (discard): A queued template can change the launch before its new
     settings are shown
   - `launcher-catalog-moves-model-highlight.md` (discard): A model catalog refresh can move the highlighted choice
   - `desktop-path-handoff-exceeds-linux-entry-limit.md` (discard): A legal inherited PATH can make managed supervisor
     startup fail
   - `macos-shift-sweep-can-miss-release.md` (discard): A macOS Shift sweep can keep running after release
   - `repeated-resizes-clear-missing-history-notice.md` (discard): Repeated height resizes can clear the lost-history
     warning without replacing the missing history
   - `empty-history-clear-keeps-missing-history-notice.md` (discard): Clearing empty scrollback leaves a restored
     missing-history notice active
   - `restored-capped-title-loses-boundary-whitespace.md` (discard): Snapshot replay trims title whitespace exposed by
     the title-length cap
   - `reconnect-replay-keeps-obsolete-event-count-fence.md` (discard): Reconnect replay can keep an obsolete restoring
     fence
   - `mixed-input-acknowledgements-misattribute-written-bytes.md` (discard): Mixed input acknowledgements can report
     discarded bytes as written
   - `snapshot-state-races-build-admission.md` (discard): Snapshot state races build admission
   - `desktop-relaunch-sleep-depends-on-path.md` (discard): The update relaunch helper can skip its wait on the restored
     PATH
   - `linux-second-launch-lost-during-quit.md` (discard): A Linux relaunch can be lost as Farhelm quits
   - `desktop-ignores-empty-login-path.md` (discard): Farhelm ignores an explicitly empty login PATH

   Type `chore`, no changelog fragment (nothing user-facing changes). The Execution field of each graduated entry
   records the change ID, bookmark and PR link and says it was discarded as decided.

2. `plan/triage-bookkeeping/02-duplicated-code-todo`: add the one Near term TODO.md entry for the 17 duplicated-code
   findings, then remove them from the queue. Executes:

   - `desktop-startup-update-duplicates.md` (duplicated code: TODO entry): Desktop startup and updates repeat shared
     logic
   - `terminal-grid-row-traversal-duplicated.md` (duplicated code: TODO entry): Terminal grid features have two row
     walkers
   - `gui-controls-repeat-shared-rendering.md` (duplicated code: TODO entry): Shared GUI controls repeat rendering and
     validation
   - `gui-focus-checks-repeat-drawn-predicate.md` (duplicated code: TODO entry): Focus checks repeat the drawn-control
     test
   - `approval-views-repeat-status-lookups.md` (duplicated code: TODO entry): Approval views repeat their waiting and
     answering checks
   - `gui-command-paths-repeat-reply-classification.md` (duplicated code: TODO entry): GUI command paths classify
     replies separately
   - `hosts-menu-and-settings-repeat-operation-logic.md` (duplicated code: TODO entry): Hosts repeats menu and operation
     logic
   - `launcher-repeats-choices-and-summary-formatting.md` (duplicated code: TODO entry): Launcher views repeat labels
     and presentation rules
   - `gui-screenshot-scenes-repeat-fixture-wiring.md` (duplicated code: TODO entry): GUI screenshot scenes repeat
     fixture setup
   - `session-list-actions-repeat-admission-and-progress.md` (duplicated code: TODO entry): Session list actions repeat
     admission and progress logic
   - `session-pane-repeats-last-known-status.md` (duplicated code: TODO entry): Session pane repeats last-known status
     calculations
   - `settings-repeat-preference-and-focus-wiring.md` (duplicated code: TODO entry): Settings repeats preference and
     editor wiring
   - `templates-repeat-lookup-and-control-wiring.md` (duplicated code: TODO entry): Template surfaces repeat lookup and
     control wiring
   - `terminal-repeats-modifier-mapping.md` (duplicated code: TODO entry): Terminal help and input repeat modifier
     mappings
   - `trash-repeats-animation-and-focus-wiring.md` (duplicated code: TODO entry): Trash repeats animation and focus
     wiring
   - `control-tests-repeat-simulated-key-events.md` (duplicated code: TODO entry): Control and approval tests repeat
     simulated key calls
   - `session-list-tests-repeat-frame-and-request-helpers.md` (duplicated code: TODO entry): Session-list tests repeat
     frame and request helpers

   Type `chore`, no changelog fragment. Write the entry from the feedback files themselves (the ledger's Assessments are
   thin), in the form under Decisions. Each graduated entry's Execution field names the TODO.md entry by its title as
   well as the change ID, bookmark and PR link.

## Fallbacks

- Decisions to log: every regrouping, every overlap with another plan's area, every use of the complexity gate, every
  outcome discarded under the unattended fallback, every review finding declined with its reason, and any criterion
  found already met or moot.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-triage-bookkeeping-log.md` in the parent directory of the checkout you run in, derived as that section
says, and the watchdog's status file is `farhelm-plan-triage-bookkeeping-watchdog.json` beside it. Resolve both to
absolute paths before you start; this file does not create either. Scratch files go in the agent scratch directory that
`AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation". Choose the narrowest checks that exercise what each PR changes.

- No Rust, JavaScript, desktop, installer or provisioning tests: the change is bookkeeping Markdown. Run `dprint check`
  on the changed Markdown files.
- Before each push, check with a script (in your scratch directory) that `review_feedback_queue/INDEX.md` names exactly
  the feedback files left in the directory, that each executed outcome is gone from `TRIAGE_OUTCOMES.md` and present
  once at the end of `TRIAGE_OUTCOMES_ARCHIVE.md`, and that every other ledger entry is untouched.

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
need is not available as a native sub-agent type (the reviewer below, for instance), `$scode-harness-shellout` supplies
the launch mechanics.

The review gate keeps the model and effort this file demands; the scope reassessment runs at your own model and effort.
The sub-agents `plans/AGENTS.md`, "Sub-agents this file requires", requires of every plan (the resume check, and the
cold reads of a blocked question and of the report) are exempt from the rules above and run as that file says: native,
read-only, at your own model.

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
  `plan/triage-bookkeeping/01-discards`, `plan/triage-bookkeeping/02-duplicated-code-todo`.
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
  ``planned in `plans/queue/triage-bookkeeping.md` `` until then. The PR description lists the outcomes it executes.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`): a PR of comment and doc corrections is `docs`, test-only changes are `test`, queue bookkeeping is
  `chore`, and a PR that fixes something users see is `fix`. Add a changelog fragment in the same commit when the type
  calls for one (root `AGENTS.md`, "Changelog fragments"): one fragment may cover a PR's user-visible fixes when they
  read as one entry, otherwise one per fix, and a `kind: none` fragment with its reason when nothing user-facing
  changed. This plan's `## Release note` section is the maintainer's answer for the release's highlights, not an
  instruction to you (`plans/.agents/executing.md`, "Executing one plan", step 8).
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides. Every other plan of the ten edits them too,
  so expect such conflicts. A conflict anywhere else (SPEC.md included) is a careful rebase
  (`.agents/careful-rebase.md`).
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The maintainer
demands exactly this reviewer, and no review swarm: a single fresh-context agent on `gpt-6.2-sol` at `high` effort,
launched through `$scode-harness-shellout` when your harness cannot serve that model natively. If it cannot be reached
when a review is due and is still unreachable after a reasonable retry, block the plan per `plans/.agents/executing.md`,
"Executing one plan" (step 10), after pushing what is consistent. Never substitute another reviewer.

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
acceptance criteria above, each having passed its review gate and each carrying its outcomes' queue removals and ledger
moves. Open, not merged. If a `## Decisions` section exists, its latest entry must also be satisfied. Then close the
plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the queue
script, write a closing entry in its log, and stop the watchdog. The report maps every outcome to its PR and says how
each ended (fixed, no change, discarded, or deferred to a TODO entry), and names every overlap with another plan's area.
Never edit `plans/` yourself.

## Release note

No release note needed.
