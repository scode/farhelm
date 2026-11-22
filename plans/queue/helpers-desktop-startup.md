# Shared helpers: desktop startup and updater

Written against main at b34ceee9f on 2026-11-21. This is a goal file for an unattended run, executed through the
planning system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it;
everything you need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over
anything this file says, and where this plan changes what they specify, it says so and you update them in the same
stack.

## The goal

This plan covers part of one TODO.md entry (bucket "Near term", directly under the bucket, not in a subsection). Four
sibling plans split that entry by area; this one owns the desktop startup and updater part. Quoted verbatim below: the
entry's heading paragraph, the two bullets this plan has a share in, and the closing line with the plan references the
planning PR adds. The entry's other bullets belong to the sibling plans and to `wide-glyph-padding-cells`.

> - **Consolidate the helpers duplicated across Farhelm.** Work through the repeated helpers from the 2026-11-21 review.
>   Reuse the existing shared module where one fits and keep each helper scoped to its actual callers. Locations below
>   are from the reviewed revision `c9575db61abb31bebe43da0f11a76712e18725c1`. Read GUI locations relative to
>   `crates/gui/farhelm-gui/src/` and desktop locations relative to `crates/farhelm-desktop/src/`.
>
>   - `desktop-startup-update-duplicates.md`. In desktop `bundle.rs:214–241` and `updater.rs:339–346,367–397`, share the
>     detached bundle wait-and-open helper with explicit wait limits and opener arguments. Keep the reopen marker and
>     update environment with their callers. In desktop `lib.rs:574–585,600–609`, share the hello discovery predicate
>     while the two startup waits keep their own child checks, deadlines and errors. In desktop
>     `window_state.rs:144–151,175–183`, share the 320×240 minimum and fits-one-monitor checks. Leave position and
>     centering to each platform. In desktop `updater.rs:1394–1405` and `updater/advisories.rs:84–99`, share the stable
>     SemVer predicate that excludes prerelease and build metadata, with each parser keeping its diagnostics. In desktop
>     `updater.rs:432–445,2558–2575`, have production and the file-backed test rig use one constructor for the three
>     state-file closures. Move the peer text escaping and empty/whitespace wording from desktop `local_host.rs:38–51`
>     and GUI `update_readout.rs:301–317` next to the shared escaping routines.
>   - `gui-screenshot-scenes-repeat-fixture-wiring.md`. The landed `goldens-fast-and-targeted` plan changed the runner.
>     The scene fixtures still need consolidation. Share one-shot dialog setup when a fake connection first has a UI
>     session. Give the helper each scene's setup callback and retain the connection, fake helm and subscription. See
>     `goldens/hosts.rs:55–85,125–146,154–176,188–209,795–817,960–988`,
>     `goldens/other_dialogs.rs:235–253,282–299,332–350`, `goldens/templates.rs:52–69,129–167,208–228` and
>     `goldens/settings.rs:122–183`. Share the session fixture's title and `TEST_NOW` activity/seen timestamps while
>     scenes supply their status (`goldens/session_list.rs:42–47,114–119,154–160`, `goldens/other_dialogs.rs:112–117`,
>     `goldens/mod.rs:301–307`). Move the upgrade-floor reinstall hover formatter into GUI `update_readout` so
>     `goldens/desktop.rs:371–384` and desktop `updater.rs:588–609` use the same words.
>
>   Read the original findings with `git show 510f3b93721ee2393bd803d2cf91d73654415fbb:review_feedback_queue/<file>`.
>   Plan: `plans/queue/helpers-desktop-startup.md`. Plan: `plans/queue/helpers-terminal-and-controls.md`. Plan:
>   `plans/queue/helpers-hosts-and-sessions.md`. Plan: `plans/queue/helpers-launcher-and-templates.md`.

This plan's share of the entry:

- the whole `desktop-startup-update-duplicates.md` bullet (six sub-items), plus copies found since the review: more
  stable-SemVer checks in `whats_new.rs`, and a third copy of the empty/whitespace wording in `session_pane/details.rs`;
- from the `gui-screenshot-scenes-repeat-fixture-wiring.md` bullet, only its last sentence: moving the upgrade-floor
  reinstall hover formatter into the GUI's `update_readout` so the golden scene and the desktop updater use the same
  words. The rest of that bullet (scene fixtures) belongs to `helpers-launcher-and-templates`.

The `terminal-grid-row-traversal-duplicated.md` bullet is in none of the four helper plans: it moved to the separate
plan `wide-glyph-padding-cells`.

Every change here is a refactor with no behavior change: the same bytes on screen, the same words, the same commands
run, the same files written.

### Where it stands (from planning research at main b34ceee9f)

No consolidation from this entry has landed since the review (`c9575db61`); every file still exists, and the duplicates
are still there at shifted lines. Re-scan each pattern rather than trusting the line numbers below or the entry's.

- **Detached bundle wait-and-open.** `crates/farhelm-desktop/src/bundle.rs`, `reopen_command` (about lines 203-245): an
  inline `/bin/sh` script that waits for the app's pid with `kill -0` without a bound, then
  `exec opener -n --env REOPENED_ENV=<id> <bundle>`, with an optional readiness file for tests; it calls
  `crate::restore_inherited_path`. `crates/farhelm-desktop/src/updater.rs`, `RELAUNCH_SCRIPT` (about 320-345) waits at
  most `RELAUNCH_WAIT_TENTHS` (600 tenths) and gives up without opening; `relaunch_command` (about 362-397) removes
  `LATEST_OVERRIDE_ENV` and the reopen marker from the environment, restores the inherited PATH, and puts the helper in
  its own process group. Tests that run the real scripts against stand-in openers: `bundle.rs`
  `the_reopen_waits_for_this_app_and_marks_the_new_one` (about 726), and `updater.rs`
  `the_relaunch_command_removes_the_latest_override`, `the_relaunch_helper_opens_the_bundle_after_the_app_exits` and
  `the_relaunch_helper_gives_up_on_an_app_that_does_not_exit` (about 1873-1980). The two differ in wait bound, opener
  arguments, environment and process group; the entry says to share the wait-and-exec helper with explicit wait limits
  and opener arguments and keep the reopen marker and update environment with their callers.
- **Hello discovery predicate.** `crates/farhelm-desktop/src/lib.rs`: the call sites of the replacement wait
  (`await_own_supervisor_answering`, called at about 656-667, defined at about 993) and of the serving wait
  (`await_managed_supervisor_serving`, called at about 682-692, defined at about 1503) each pass the same closure:
  `discover_local_supervisor(&farhelm, &state_dir)` compared with `LocalSupervisorDiscovery::Answering`. The waits keep
  their own child checks, deadlines and errors.
- **Saved-window size check.** `crates/farhelm-desktop/src/window_state.rs`: `usable_rect` (about 143-163) and the
  Wayland branch of `restore_rectangle` (about 166-183) both check the 320×240 minimum and that the size fits one
  monitor. Position checks and centering stay per platform.
- **Stable SemVer predicate, now three copies in this crate.** `!parsed.pre.is_empty() || !parsed.build.is_empty()` in
  `updater.rs` `latest_from_site` (about 1404), `updater/advisories.rs` (about 89), and twice in
  `crates/farhelm-desktop/src/whats_new.rs` (about line 28, combined with the macOS check, and about 110, inside the
  feed filter). Each parser keeps its own diagnostics.
- **Updater state-file closures.** Production `updater::start` (about 434-448) builds `automatic_enabled`,
  `read_probe_failure_since` and `write_probe_failure_since` over the state file; the test rig's `Rig::with_state_file`
  (about 2561-2585) builds the same three. The point of sharing is that the tests then exercise the production wiring.
- **Peer text formatter.** `crates/farhelm-desktop/src/local_host.rs` `display_peer` (about 34-52) and
  `crates/gui/farhelm-gui/src/update_readout.rs` `shown_build` (about 301-317) are the same function: `(empty)`, the
  escaped text, or `(whitespace only: N characters)`. Both doc comments say the other keeps a copy because the GUI crate
  cannot call the desktop crate. The shared escaping routines they both use are in `crates/farhelm-proto/src/text.rs`
  (`is_presentation_unsafe`, `notation`, `escaped`, about 71-135), which both crates already depend on. A third copy of
  the empty/whitespace half, new since the review: `crates/gui/farhelm-gui/src/session_pane/details.rs` `placeholder`
  (about 331-344), which returns `(empty)` or `(whitespace only: N characters)` under the same "whitespace and not
  presentation-unsafe" test (its doc comment calls them "The old app's `display_peer` markers") and `None` otherwise.
  Include it if it is the same rule (for example through a marker function the peer formatter also uses); otherwise
  leave it and log why.
- **Upgrade-floor reinstall hover.** `crates/farhelm-desktop/src/updater.rs`, the readout builder (about 602-623): the
  `ReinstallReason::UpgradeFloor` advice, then the idle hover, then
  `Reinstall it by running this in a terminal: {REINSTALL_COMMAND}` (the constant at about line 58).
  `crates/gui/farhelm-gui/src/goldens/desktop.rs` `reinstall_tooltip` (about 371-384) is a hand-copied version of those
  words for the golden scene, with the command spelled out; its doc comment explains the copy (the GUI crate cannot
  depend on the desktop crate). The desktop crate already uses `farhelm_gui::update_readout`.

### Acceptance criteria

1. Each sub-item above has one shared helper (or is skipped per "Decisions already made", with the reason logged and
   reported), placed in an existing shared module where one fits and scoped to its actual callers.
2. No behavior changes: the reopen and relaunch helpers run the same effective shell behavior (bound, opener arguments,
   environment, process group, readiness file) as before; startup waits keep their own checks, deadlines and errors;
   window restore arithmetic is unchanged; each SemVer parser keeps its diagnostics; the readout's words are
   byte-identical.
3. The updater and bundle tests listed above pass unchanged in what they assert (see "Planner proposals", updater
   tests).
4. Doc comments that say "keeps a copy" or "copied rather than called" are corrected where the copy is gone.
5. Every GUI golden stays byte-identical (see "No visual change").
6. This plan's part of the TODO entry is removed by the rule under "PR discipline".

## Decisions already made

Settled with the maintainer during planning (2026-11-21):

- **Split by area into four plans**, of which this is one; siblings are `helpers-terminal-and-controls`,
  `helpers-hosts-and-sessions` and `helpers-launcher-and-templates`. This plan waits on nothing.
- **Skipping a sub-item:** you may skip a sub-item you judge not worth sharing (for example two copies that differ in a
  way sharing would blur), recording why in the working log as a DECISION and in the report.
- **New copies are in scope by default**, in particular the third stable-SemVer copy in
  `crates/farhelm-desktop/src/whats_new.rs`. Re-scan each pattern rather than trusting the old line numbers.
- **The helm's stable-version check is out of scope:** the similar test in
  `crates/farhelm-helm/src/provisioning/uninstall_refusal.rs` stays as it is, and no helper crosses crates for it.
- **No visual change:** byte-identical GUI goldens are the oracle. A golden that changes is a red flag, not something to
  accept (see "No visual change").
- **Commit types:** every PR is `refactor:` (or `test:` for a test-only one) and behavior-free, so none needs a
  changelog fragment.
- **Review gate:** a single fresh-context `gpt-6.2-sol` agent at `high` effort.
- **Release note:** `No release note needed.`
- **No-workhorse mode:** the plans default, stated on the planning page.

## Requirement sources

**The maintainer's request and decisions:** the TODO entry and the decisions above.

**Binding repository constraints:**

- Root `AGENTS.md`, "Conventional Commits", "Changelog fragments", "TODO.md and FLAKES.md", "Tests and validation", and
  its "Instructions by path" for every path you touch (`.agents/validation.md` for any test).
- `.agents/test-authoring.md` for every test added or changed.
- `.agents/validation.md`: the desktop app entry (`cargo nextest run -p farhelm-desktop` through the recorder, and its
  two clippy runs, since the desktop crate is outside `default-members`), and the GPUI goldens entry.
- SPEC.md's update and startup sections hold as they are; nothing here may change what they specify.

**Planner proposals:**

- **Updater tests:** the updater is brick-sensitive. Rely on the existing updater tests, which run the real scripts; add
  a focused test only where a shared helper loses coverage. Every PR touching `updater.rs`, `updater/advisories.rs`,
  `bundle.rs` or `whats_new.rs` runs the desktop app's tests.
- Put the shared reopen helper where both callers can reach it inside the desktop crate (`bundle.rs` already holds the
  reopen marker `REOPENED_ENV` that `updater.rs` imports). Pass the wait bound as an explicit option (none for the
  reopen, `RELAUNCH_WAIT_TENTHS` for the relaunch) and the opener's extra arguments explicitly; keep the `env_remove`
  calls, the `--env` marker and `process_group(0)` at the callers, as the entry says.
- One `pub(crate)` stable-version predicate in the desktop crate for the four desktop sites.
- Move `display_peer`/`shown_build` into `crates/farhelm-proto/src/text.rs` beside `escaped`, under one name both crates
  call, with `session_pane/details.rs` `placeholder` using its marker half if it is the same rule.
- Move the upgrade-floor reinstall hover into `farhelm_gui::update_readout` as a function the desktop updater calls and
  the golden scene calls. Where the reinstall command constant ends up (moved to the GUI crate, or passed in) is your
  call; the words, including the command, must stay byte-identical.

## Outline

1. `plan/helpers-desktop-startup/01-detached-reopen-helper`: the shared wait-and-open helper for `bundle.rs` and
   `updater.rs`; the existing tests that run both real scripts keep passing. `refactor:`, no fragment.
2. `plan/helpers-desktop-startup/02-startup-checks`: the hello discovery predicate in `lib.rs` and the saved-window size
   predicate in `window_state.rs`. `refactor:`, no fragment.
3. `plan/helpers-desktop-startup/03-stable-version`: the stable SemVer predicate for `updater.rs`,
   `updater/advisories.rs` and both `whats_new.rs` sites. The predicate has no direct test today, so it gets a short
   one. `refactor:`, no fragment.
4. `plan/helpers-desktop-startup/04-updater-state-file`: one constructor for the three state-file closures, used by
   `updater::start` and `Rig::with_state_file`. `refactor:`, no fragment.
5. `plan/helpers-desktop-startup/05-peer-text`: the peer text formatter moved into `farhelm-proto`'s text module, used
   by `local_host.rs` and `update_readout.rs` (and `session_pane/details.rs` `placeholder` if it fits). `refactor:`, no
   fragment.
6. `plan/helpers-desktop-startup/06-reinstall-hover`: the upgrade-floor reinstall hover moved into the GUI's
   `update_readout`, used by the desktop updater and `goldens/desktop.rs`; this plan's part of the TODO entry removed.
   `refactor:`, no fragment.

Small neighbouring sub-items may share a PR, and a large one may split, as long as each PR stays one reviewable
refactor; log the shape you chose.

## Fallbacks

- A sub-item whose copies turn out to differ in a way sharing would blur (for example the reopen and relaunch scripts,
  if a shared script cannot keep both exactly): skip it, log a DECISION, and say so in the report.
- If a sub-item has already been consolidated on main by other work when you reach it, record that and move on.
- **Concurrency.** The siblings `helpers-terminal-and-controls`, `helpers-hosts-and-sessions` and
  `helpers-launcher-and-templates` touch disjoint files except: `helpers-launcher-and-templates` also adds a function to
  `crates/farhelm-proto/src/text.rs` (a display-unit iterator for folder labels), and touches other `goldens/` files,
  not `goldens/desktop.rs`; `helpers-hosts-and-sessions` edits `session_pane/details.rs` (the pane status projections),
  where `placeholder` lives. These rebases are mechanical; keep both sides. Any other plan that lands first and touches
  these files gets a careful rebase (`.agents/careful-rebase.md`).
- **Fence: no visual change.** See "No visual change" under "How to run".
- **Fence: the live install.** Never run, rebuild, install or restart the maintainer's own Farhelm to try the updater or
  startup code (root `AGENTS.md`, "The live install is off-limits"); the desktop tests use stand-ins.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-helpers-desktop-startup-log.md` in the parent directory of the checkout you run in, derived as that
section says, and the watchdog's status file is `farhelm-plan-helpers-desktop-startup-watchdog.json` beside it. Resolve
both to absolute paths before you start; this file does not create either. Scratch files go in the agent scratch
directory that `AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation". Choose the narrowest checks that exercise what each PR changes: for this plan that is mainly the desktop
app's nextest selection (filtered to the touched modules while iterating, the whole `-p farhelm-desktop` selection
before each PR is final), its two clippy runs, `farhelm-proto`'s and `farhelm-gui`'s tests for PRs 05 and 06, and the
goldens for PRs that touch `update_readout.rs` or `goldens/`. Apply `.agents/test-authoring.md` to every test you add or
change, and run `python -B scripts/check-test-sleeps.py` as `docs/test-sleep-check.md` prepares it when Rust tests
change. Never take a state-changing action against the live Farhelm install on this machine (root `AGENTS.md`, "The live
install is off-limits").

### No visual change

These PRs must not change what the app draws, so there is no visual review. Byte-identical GUI goldens are the oracle:
run the goldens (`python3 scripts/check-gui-goldens.py` through the recorder, `.agents/validation.md`) for every PR that
touches drawing code or `goldens/`, in compare mode, never `--update`. Any golden that changes must be investigated and
the change reverted, unless it is a pre-existing nondeterminism, which you show by reproducing it on the PR's base and
log (with the scene and the evidence) in the working log and the report. Never publish new golden pictures from this
plan.

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
without costing quality. Concurrent agents that might otherwise conflict can work in separate jj workspaces whose
results you merge back; you still own the stack and every PR. Launch a sub-agent natively, with model and effort set
explicitly where the mechanism allows; when the model you need is not available as a native sub-agent type (the reviewer
below, for instance), `$scode-harness-shellout` supplies the launch mechanics.

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

- One reviewable commit, stable bookmark and draft PR per step, in a single linear stack, using the `jjstack` skill. The
  stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per `plans/.agents/executing.md`,
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks: `plan/helpers-desktop-startup/<nn>-<short-name>`
  as the outline names them.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- **The shared TODO entry.** Four sibling plans (`helpers-desktop-startup`, `helpers-terminal-and-controls`,
  `helpers-hosts-and-sessions`, `helpers-launcher-and-templates`) split the TODO entry "Consolidate the helpers
  duplicated across Farhelm.", and which of them lands last cannot be known. So each plan's last PR deletes only that
  plan's own part of the entry: its group bullets, and for a bullet split between plans only its own sentences (the
  `templates-repeat-lookup-and-control-wiring.md` bullet: the last two sentences, from "Share physical keypad digit
  lookup" to the end, are `helpers-terminal-and-controls`'s, the rest `helpers-launcher-and-templates`'s; the
  `gui-screenshot-scenes-repeat-fixture-wiring.md` bullet: the reinstall hover formatter sentence is
  `helpers-desktop-startup`'s, the rest `helpers-launcher-and-templates`'s). The text of a sub-item the plan skipped, or
  found already consolidated, is deleted with the rest of its share; the report carries the skip and its reason. The
  same PR deletes the plan's own `Plan:` reference from the entry's closing line. The plan that removes a split bullet's
  last part deletes the bullet. If that deletion leaves the entry with no group bullets at all, the same PR also deletes
  the entry's heading paragraph and its closing "Read the original findings" line with the plan references. While any
  bullet remains, the heading stays; that includes the `terminal-grid-row-traversal-duplicated.md` bullet, which the
  separate plan `wide-glyph-padding-cells` removes under this same rule. Apply this rule to `TODO.md` as it is on
  `main@origin` when you rebase for delivery; on a rebase conflict in this entry, keep every deletion from both sides
  and apply the rule again.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`). Every PR in this plan is `refactor:` (or `test:` if test-only) with no user-visible effect, so none adds
  a changelog fragment (root `AGENTS.md`, "Changelog fragments"). This plan's `## Release note` section is the
  maintainer's answer for the release's highlights, not an instruction to you (`plans/.agents/executing.md`, "Executing
  one plan", step 8). Never edit that section.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides.
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
findings file in the scratch directory, and the acceptance criteria (this file's goal, acceptance criteria, decisions
and outline). Tell it that every PR here must be behavior-free, so any behavior difference is a finding. When a PR
changes tests or their fixtures, include the full text of `.agents/test-authoring.md` verbatim in the prompt, as root
`AGENTS.md` requires.

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
alternatives considered: in particular those named under Fallbacks above, every sub-item you skip, and every review
finding you decided not to follow. The maintainer will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the maintainer's decisions and the repository constraints still
hold. Wording in this file that turns out inaccurate (a line number, a file name, a test name) is recorded in the
working log with the true value; never edit this plan file.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the
maintainer's decision; a review finding or a log entry is not authorization. If no fallback covers it, record the
concrete tradeoff and continue with independent work. Block per `plans/.agents/executing.md`, "Executing one plan"
(step 10) only for what cannot proceed without the maintainer, after pushing every PR that can be finished.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists, one per step in the order the outline gives (or as the
outline's fallbacks allow), together meeting the acceptance criteria above, each having passed its review gate, with
every GUI golden unchanged, and the last removing this plan's part of the TODO.md entry by the rule under "PR
discipline". Open, not merged. If a `## Decisions` section exists, its latest entry must also be satisfied. Then close
the plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the queue
script, write a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.

## Release note

No release note needed.
