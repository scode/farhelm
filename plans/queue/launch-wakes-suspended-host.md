# A launch on a suspended host resumes it first

Written against main at 3617ee450 on 2026-11-21. This is a goal file for an unattended run, executed through the
planning system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it;
everything you need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over
anything this file says, and where this plan changes what they specify, it says so and you update them in the same
stack.

## The goal

This plan covers one TODO.md entry (bucket "Near term"). Verbatim:

> **A launch onto a suspended host should resume it first.** SPEC.md "Topology" says launching on a suspended host
> resumes it and waits for a fresh connection, as the browser app did; the GPUI app refuses and asks the user to resume
> (interim, decided 2026-11-17). Tensorlake suspends an idle sandbox after 30 minutes, including once the app quits, so
> this is the usual first launch after a break. Scoped at the migration's end at about 660 production and 1,340 test
> lines: the launcher's wake over pushed host state (Resume, wait for a fresh connection within the same minute, retry
> while another operation holds the host, fail on a new suspension), the preparation pill, the fresh-checkout folder
> allocation once the host is ready, quick launch, and "this launch is waking that host" refusing Hosts' Suspend, Resume
> and Settings and a session's own Suspend and Resume. The old app's 13 launch-after-wake cases (tensorlake-lifecycle)
> are the spec. Design the wake with an opus consult. Plan: `plans/queue/launch-wakes-suspended-host.md`.

The browser app shipped this in v0.39.0 (its changelog: "A launch that wakes a suspended sandbox or paused container now
waits for another Suspend or Resume of it to finish…"); the desktop app lost it in v0.40.0. The Opus design consult the
entry asks for was done during planning; its result is the design below, which the maintainer approved.

### Where it stands (from planning research at main 3617ee450)

- **The refusal.** `host_refusal` (`crates/gui/farhelm-gui/src/launcher/request.rs`, about line 418) returns
  `Refusal::ResumeHost` for a `resumable` host with a provider; wording from `ProviderUi::suspended_launch_state`
  (`environment_ui.rs`, about 188) and `resume_host_advice` (`request.rs`, about 453). `resumable()`
  (`crates/gui/farhelm-gui/src/hosts/offers.rs`, about 396) matches `Suspended { .. }` or
  `Stopped { waiting_for_place: false, .. }`. Tests pinning the refusal:
  `a_suspended_environment_names_the_resume_needed_before_launch` (`launcher/request_tests.rs`) and
  `a_suspended_quick_launch_asks_to_resume_its_named_host` (`templates/dispatch_tests.rs`).
- **Two NOTEs wait for this work:** `hosts/lifecycle.rs` (about 46-49) and `session_list/environment.rs` (about 70-74)
  say Suspend/Resume should be refused "while this window's own launch targets or is waking the same host … since the
  launcher exposes no predicate for its pending target yet". `hosts.rs`'s `launch_in_flight` (about 1063) counts only a
  create in flight or an accepted environment create.
- **The launcher's wait machinery** (`launcher/launch.rs`): after a press the launcher is hidden in a per-UI-session
  `LaunchSlot` (`Held { launcher, waiting, progress, run }`, `Waiting::{Preparing, Reply, User}`). An accepted
  environment create already waits on pushed state (`RunWait`, driven by `follow_run` on every store change), the
  progress pill reads the store (`launcher/pill.rs`), `busy()` refuses New and quick launch while the slot is busy, and
  refusals come back through `come_back` under the moved-on rule.
- **The helm side is already done.** `POST /api/hosts/{id}/resume` (`crates/farhelm-helm/src/environment/lifecycle.rs`)
  works for both providers. `HostBusy` is a 409 with the helm-only `x-farhelm-host-busy` header, decoded by the client
  as `HelmMarker::HostBusy` (`crates/farhelm-proto/src/ui_channel.rs`). A Resume still running at the 20 s reply bound
  answers 504 "carries on" while the host keeps showing `TensorlakeChanging { direction: Resuming }` (used for every
  provider); a Tensorlake Resume can take up to 120 s. SPEC_impl.md "Tensorlake hosts" (about 6807-6811): "so a launch
  waking its host can retry on it and on nothing else. The desktop launcher does not wake a suspended host yet."
- **SPEC.md** "Topology" (about 356-365, 381-384), "Creation" (about 1454-1468, 1478), "Managed checkouts" (about
  1804-1808) and "Container sessions" (about 5240-5242) already describe the target behavior, with interim and "Once
  implemented" wording this plan removes.
- **The old browser implementation**, readable with `git show 24bc0580:<path>` (the GPUI parity pin; the browser UI was
  removed in `6ef88c9a4` and `1eb4e3751`, unchanged since that pin): `crates/farhelm-ui/src/api.rs` (about 5669-5915:
  `resume_host_connected`, `post_lifecycle`, `WAKE_WAIT`, `WakeBudget`, `WakeWatch`/`WakeStep`),
  `crates/farhelm-ui/src/list/create_form.rs` (`HostResumeProgress`, the woken-connection wait, the allocation notice,
  the submit path's wake branch, about 6195-6330), `crates/farhelm-ui/src/ops.rs` (the `launch_target` refusal, words "A
  launch on this host is in progress; try again once it finishes"), and the 13 cases in
  `e2e/tests/tensorlake-lifecycle.spec.ts` (`docs/gpui/parity/launcher.md` lists them as deferred). Reuse its
  user-facing words verbatim unless a desktop-app style rule says otherwise.

### The 13 cases (the entry's spec)

1. A woken launch creates without revealing the launcher, taking focus or making the window modal; an open quick
   switcher stays open.
2. An uninterrupted quick launch opens its session, then its naming dialog, after the wake (the wake is not a user
   answer).
3. A quick launch retries a busy Resume until accepted; fails at once (long before 60 s) when the host is suspended
   again; does not mistake the stale starting suspension for a new one.
4. A managed Replace with claims its source from acceptance through the wake and preview; a preparation refusal releases
   the source before the launcher comes back.
5. An existing-folder launch resumes before creating: no "unavailable" flash, no early create, and the create uses the
   fresh connection.
6. A successful Resume stays honest while the displayed host catches up (no unavailable warning in between).
7. A host another surface is suspending or resuming does not make the launcher available.
8. Quick launch shows the preparation pill, not the create-progress pill, while resuming; a refusal reveals the launcher
   after exactly one create.
9. A failed Resume refuses an existing-folder launch with "Not launched." and creates nothing, never with the
   unknown-outcome wording.
10. A create delayed after the Resume names the host in its progress pill without a stale "(Suspended)" suffix.
11. A suspended fresh checkout takes one press (form, recent-enter, typed-taken, quick, number, lost-race): it waits for
    the connection and its own fresh preview, uses the occupied-name suffix search, and the pill and busy state last
    through the preview wait.
12. A quick launch refused without an error after its wake (the host became unavailable during the preview) brings the
    launcher back visibly, with its reason and without a create.
13. A woken fresh checkout waits at most 60 s to see its host connect; on timeout the launcher comes back with the
    timeout words and a later connection creates nothing; a connection within the limit creates even if the whole wait
    passes 60 s.

### Acceptance criteria

1. New, Clone, Restart with, Replace with, quick launch, template number keys and quick-launch sequences on a resumable
   host resume it (or start it, when stopped) and create the session with one press, without revealing the launcher, for
   existing folders and fresh GitHub checkouts alike. A plain row Restart on a suspended host is out of scope.
2. The 13 cases above hold, each covered by a deterministic `gpui::test` with the fake helm (pushed host states,
   simulated ticker clock), as the existing `dispatch_tests` and `hosts/environment_tests` do.
3. A busy Resume is resent each second; any other refusal fails the launch at once with "Not launched. Resuming the host
   failed: ‹words›"; a Resume that finishes elsewhere ends the wait successfully. Suspended again, a newly appearing
   resume failure, the row removed, or the installation changed fail at once with their words.
4. The connection budget is 60 s, paused while this wake's accepted Resume is still running (the host shows
   `Changing { Resuming }`). The helm already bounds that phase (SPEC_impl.md "Tensorlake hosts": Resume waits at most
   120 s and releases the hold); an extra hard ceiling is optional (log whether you add one).
5. The preparation pill shows at once at the press ("Resuming ‹host›, then connecting… the session starts when it's
   ready", "Starting ‹host›, then connecting…" for a stopped host), with the host name captured without a phase suffix,
   and stays through the preview wait; it gives way to the ordinary progress pill under the existing two-second rule.
6. While this window's launch targets or wakes host H, H's Suspend and Resume in Hosts and a session's Suspend, Resume
   and Stop and suspend on H are disabled and refused with "A launch on this host is in progress; try again once it
   finishes"; the whole wake counts as `launch_in_flight` (Add host, setup, Re-run, removal, Update and host Settings'
   destination save refused) and `busy()` is true. Both NOTEs are removed.
7. A managed checkout on a resumable host shows "The folder is picked automatically when the host is ready" (or "… named
   after the session when the host is ready" once a name is typed) instead of a path and does not wait for a preview
   before Launch; after the connection, the hidden launcher takes a fresh preview on the new connection and creates with
   it.
8. No helm, protocol or supervisor change.
9. SPEC.md "Topology", "Creation", "Managed checkouts" and "Container sessions" and SPEC_impl.md "Tensorlake hosts" say
   what the desktop app now does; "Creation"'s two 60 s waits become the one wait. `docs/gpui/parity/launcher.md`
   records the 13 cases as done. The TODO entry is removed.

## Decisions already made

Settled with the maintainer on the 2026-11-21 planning page:

- **The design below**, including the mockup words, is approved as shown.
- **A launch's Resume may hold up the window's other helm actions for up to 20 s**, as Hosts' Resume already does; the
  general fix stays with the existing "One slow host holds up every other action" TODO entry. No helm change here.
- **Stopped hosts wake too:** a launch also starts a stopped sandbox or container that offers Resume (`resumable()`),
  not only a suspended or paused one. Its pill says "Starting ‹host›, then connecting…".
  `Stopped { waiting_for_place: true }` keeps today's refusal. SPEC.md "Container sessions" loses its "Until launches
  can resume an environment" qualifier.
- **SPEC "Creation" rewording:** its two 60 s waits for a fresh checkout become one (the desktop app has a single host
  reader), keeping the timeout message.
- **Review gate:** a single fresh-context `gpt-6.2-sol` agent at `high` effort, plus the mandatory Opus visual review
  (see "Visual review").
- **Release note:** `No release note needed.`
- **No-workhorse mode:** the plans default, stated on the planning page.

## Requirement sources

**The maintainer's request and decisions:** the TODO entry, the 13 cases, and the decisions above.

**Binding repository constraints:** SPEC.md and SPEC_impl.md as cited; root `AGENTS.md`, "Code rules": every provider
decision goes through the exhaustive provider UI module (`ProviderUi`) or an exhaustive method, never a provider
comparison in shared code (read the module-doc map in `crates/farhelm-helm/src/environment/mod.rs`); SPEC_impl's
"Parking is forbidden" rule for the deterministic scheduler (use the ticker's simulated-clock deadlines);
`.agents/test-authoring.md`; the GPUI goldens check; root `AGENTS.md`, "Conventional Commits", "Changelog fragments",
"Tests and validation", and its "Instructions by path" for every path you touch.

**Planner proposals** (the design consult):

- **The wake lives in the desktop app, in the launcher's slot, not in the helm.** The SPEC states it per window, and the
  helm already has the provider-neutral Resume, the busy marker and pushed host state. A helm-side "wake and create" was
  considered and rejected: a new helm task, a run shape for ordinary hosts and a helm-side preview, for a race the SPEC
  already handles.
- **Where the wake lives in the slot (your close call; log it):** either a new `Waiting::Waking`, or (planning review's
  suggestion) the existing `Waiting::Preparing` with a `wake: Option<Wake>` record, mirroring how `Reply` carries
  `run: Option<RunWait>`. `Preparing` already means "hidden, busy, nothing sent yet, will start through `start` or come
  back through `reveal_prepared`", and `busy()`, `holding()`, `still_preparing`, `abandon_prepared` and
  `reveal_prepared` already treat it that way, so the hand-off below works unchanged. Either way the `Wake` record
  (host, starting host view, install, mark, window, phase, budget, resend deadline) outlives the hand-off until `start`
  replaces the held launcher, which is what keeps `launch_in_flight` and `busy()` true through the preview wait (case
  11).
- **Where the wake starts:** for a press, in `Launcher::submit`, before `launch::start`, when the draft's host is
  resumable. For a quick launch, before `start_or_wait_for_folder`'s folder-default and preview steps (on a suspended
  host the history cannot be read yet: `history_key` is `None` unless the host is connected).
- **State machine:**
  - _Asking_: the Resume command is out (budget paused). Done or Accepted, or a 504 "carries on" with no marker →
    _Resuming_. `HostBusy` → _Busy_ (arm a 1 s resend; budget counting). Any other refusal → fail.
  - _Busy_: on the resend deadline → _Asking_; on a store change → evaluate.
  - _Resuming_: budget paused while the host shows `Changing { Resuming }` (up to the ceiling), counting otherwise; on a
    store change → evaluate.
  - _Ready_: connected. **Hand off to the existing hidden-launch pipeline; do not build a new one.** Re-enter
    `Launcher::start_or_wait_for_folder` (`launcher/view.rs`, quick launch: it waits for New's folder default from the
    now-readable history, then the preview) or `Launcher::submit_or_wait_for_preview` (`launcher/view/checkout.rs`, a
    press), with the press's original mark. Those already ask for the preview, arm `checkout.armed`, settle through
    `settle_waiting_launch` (which follows the occupied-name advance) and go through `submit`, which re-runs the refusal
    and the request build against the new connection, asks the sandbox question, and captures the pill's host name from
    the now-connected row (case 10). A launch that does not start comes back through `launch::reveal_prepared` (case
    12). Never call `launch::start` directly from the wake. The one generalization this needs: `Armed.unseen` currently
    gates three things in `settle_waiting_launch` (the numbered-name follow, starting with no layer open, and
    `reveal_prepared` on a non-start); a woken press from a shown launcher (New, Clone, Replace with) needs all three
    but must not get the quick-launch naming dialog, so split it into "held out of sight" (true for every woken launch)
    and keep the quick-launch flag for naming alone.
  - _evaluate_: row gone → fail; installation changed → fail; connected → Ready; any Changing → wait; a
    Suspended/Stopped reading equal to the starting one → wait; same marks with a failure newly set → fail with the
    provider's could-not-resume words; any other Suspended/Stopped, or any after a connection phase was seen → fail
    "suspended again"; other phases → note that a connection phase was seen, and wait. Terminal states such as Deleting
    or Retired fail at once.
  - Budget exhausted → "did not connect within 60 seconds" after an accepted Resume, or "stayed busy … for 60 seconds"
    if never accepted. The comparison is over `HostStateView` variants, not providers, so it can live in the launcher;
    provider-specific words go through new `ProviderUi` methods. `follow_run` becomes "follow the slot's wait".
- **Preparation pill:** a `WakePill` beside `ProgressPill` in `launcher/pill.rs`, same layer slot
  (`layers::show_launch_pill`), with a spinner and `Role::Status`.
- **Concurrent operations, derived from the held launcher; no new stored state.** While held, the launcher's draft
  cannot change, and `draft.host` is the target in all four modes (Replace with and Restart with keep the source's
  host). So `launch::targets_host(H, cx)` is: the slot holds a launcher that is waiting for its reply or waking, whose
  `draft.host == Some(H)` and `draft.target.is_none()`. Use it in `hosts/lifecycle.rs` and
  `session_list/environment.rs`. The Replace with / Restart with source claim (case 4) is likewise "the held launcher's
  mode source equals this session key while the wake record is set", one more term in `session_list::menu::refused`; in
  the Reply phase the source's key is already in flight in the operation store. Do not use `outcome::start_clock` for
  the claim: `Outcomes::prune` drops clocks with nothing in flight on the session key, which is the case during a wake.
  `hosts::launch_in_flight` gains the wake (one slot predicate, or `environment_create_running` widened to "the slot
  holds a launch the operation store does not see"). Collisions with other windows or the CLI stay with the helm's
  lifecycle lock, which the wake answers by retrying.
- **Left to you (log each):** which operation key the wake's Resume uses (the host's key with `Work::Resume` makes
  Hosts' row show "Resuming…" for free but leaves a refusal under that row unless forgotten, as `hosts/lifecycle.rs`
  does; a launcher-private key keeps it out of Hosts), and where the preparation pill's id lives so it is always taken
  down (`hide_progress` hides only `Progress.shown`; a create answering within two seconds and a come-back after a
  failed wake must also remove the wake pill).
- **Dead code to remove in PR 2:** once resumable hosts wake, `Refusal::ResumeHost`, its `view.rs` hover arm (about
  2649), `LaunchDraft::resume_host_advice`, `ProviderUi::suspended_launch_state` and the `request.rs` module note (about
  line 15) lose their last user. `Stopped { waiting_for_place: true }` is not `resumable()` and already gets the plain
  "Choose a connected host before launching"; that stays.
- **Ordering risks** that need their own deterministic tests: the 504 arriving before any Changing phase is pushed (a
  container's SSH check), pushes suppressed while this window's own Resume command is out, and a stale starting reading.

**Mockup (approved):**

```
Composer, existing folder, suspended host:   Launch enabled, hover "Launch: start the session" (no Resume advice)
Composer, managed checkout, suspended host:  Folder: "The folder is picked automatically when the host is ready"

After the press (launcher hides; capsule bottom-centre, same slot as the launch progress pill):
   ( ◌ Resuming devbox-sandbox, then connecting… the session starts when it's ready )
   → once the create is out: ( Launching "fix-tests" on devbox-sandbox… 3 s )

Failure: launcher returns with its draft and, beside Launch:
   Not launched. The host was suspended again while it was waking; resume it and launch again.

Meanwhile in Hosts:  devbox-sandbox  [Suspend] [Resume]   ← greyed, hover:
   "A launch on this host is in progress; try again once it finishes"
```

## Outline

1. `plan/launch-wakes-suspended-host/01-launch-targets-host`: `fix:` add `targets_host` (the create phase only), wire
   the Hosts and session controls, remove both NOTEs, tests in `hosts/environment_tests.rs` and the session-list
   environment tests. Changelog fragment (`kind: fixed`).
2. `plan/launch-wakes-suspended-host/02-wake-before-launch`: `feat:` the wake record and state machine, `WakePill`, the
   hand-off to the existing hidden-launch pipeline (with the `Armed.unseen` split), `host_refusal` no longer refusing
   resumable hosts and the dead refusal code removed, the wake in `launch_in_flight`/`busy`/`targets_host`, the Replace
   with / Restart with source claim, existing-folder New, Clone, Restart with, Replace with and quick launch (including
   a woken quick launch waiting for New's folder default once the history is readable), stopped hosts. Ports cases 1-10
   and the existing-folder variant of 12; inverts the two refusal tests; a golden for the preparation pill. SPEC.md
   "Topology" and "Container sessions", SPEC_impl.md "Tensorlake hosts", `docs/gpui/parity/launcher.md`. Changelog
   fragment (`kind: changed` or `added`, your call).
3. `plan/launch-wakes-suspended-host/03-wake-fresh-checkout`: `feat:` the composer's allocation notice, the
   `checkout_refusal` exception for a resumable host, and the fresh-checkout timeout words for PR 2's existing 60 s
   budget (no second wait mechanism: the preview, occupied-name advance and create come from the pipeline PR 2 hands off
   to). Ports cases 11, 13 and the checkout variant of 12; a golden for the composer state. SPEC.md "Managed checkouts"
   and the "Creation" rewording. Edits PR 2's fragment. Removes the TODO entry.

## Fallbacks

- PR 1 may be folded into PR 2 if keeping it separate would mean code that PR 2 rewrites (log it).
- If a helm change turns out to be necessary after all, run the scope reassessment first; the maintainer rejected the
  helm-side Resume change for this plan.
- Whether to add a hard ceiling, whether the pill shows elapsed seconds, and the slot shape (above) are yours to settle
  (log them).
- A visible New with an untouched pending folder default on a suspended host has the same folder-default question as a
  woken quick launch (the history arrives only after the wake, while the launcher is hidden): handle it the same way,
  through the re-entered pipeline.
- If `triage-gui-misc.md` or `triage-gui-sessions.md` has landed changes in the same launcher, Hosts or session-list
  files by the time you start, rebase carefully onto them (`.agents/careful-rebase.md`); they change nearby code, not
  this behavior.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-launch-wakes-suspended-host-log.md` in the parent directory of the checkout you run in, derived as that
section says, and the watchdog's status file is `farhelm-plan-launch-wakes-suspended-host-watchdog.json` beside it.
Resolve both to absolute paths before you start; this file does not create either. Scratch files go in the agent scratch
directory that `AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation". Choose the narrowest checks that exercise what each PR changes. Apply `.agents/test-authoring.md` to every
test you add or change, and run `python -B scripts/check-test-sleeps.py` as `docs/test-sleep-check.md` prepares it when
Rust tests change. Never take a state-changing action against the live Farhelm install on this machine (root
`AGENTS.md`, "The live install is off-limits").

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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks:
  `plan/launch-wakes-suspended-host/01-launch-targets-host`, `plan/launch-wakes-suspended-host/02-wake-before-launch`,
  `plan/launch-wakes-suspended-host/03-wake-fresh-checkout`.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- The last PR of the stack removes the TODO.md entry this plan covers ("A launch onto a suspended host should resume it
  first"), as root `AGENTS.md`, "TODO.md and FLAKES.md", requires.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments"). A later PR of this stack that amends work an earlier PR of it introduced edits that PR's fragment rather
  than adding a second one. This plan's `## Release note` section is the maintainer's answer for the release's
  highlights, not an instruction to you: each PR still leaves the fragment its type calls for, describing in user-facing
  words what was built, and saying in its context paragraphs where what was built departs from the release note
  (`plans/.agents/executing.md`, "Executing one plan", step 8). Never edit that section.
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
and outline). When a PR changes tests or their fixtures, include the full text of `.agents/test-authoring.md` verbatim
in the prompt, as root `AGENTS.md` requires.

The maintainer's standing rule for this reviewer (2026-11-17): its findings are inputs, not orders. Take a correctness
finding (a bug, a missed case, a wrong claim) after checking it yourself. A finding that would add code, a new
mechanism, a new test layer or another round of hardening is declined by default unless you judge it plainly necessary;
never let a review grow a PR. Log each finding you decline as a DECISION with its reason. Do not write a launch command
here or in the log; the harness-shellout skill owns launch mechanics.

### Visual review

The maintainer also demands a visual review for every PR that changes what the desktop app draws: a fresh-context
`claude-opus-5-5` agent at `high` effort, launched natively where your harness can (otherwise through
`$scode-harness-shellout`), separate from the reviewer above. Give it headless renders of every changed scene in both
the light and the dark appearance (the GUI golden scenes, regenerated with
`python3 -B scripts/check-gui-goldens.py --update`, and the golden changes page that run writes), the design this file
describes (its mockup text and wording are the agreed design), and the PR's diff. It must be satisfied that the rendered
result matches the intent of the agreed design; loop until it reports no mismatch. A mismatch with the agreed design is
binding and is fixed; any other observation it makes is an input, handled like the reviewer's findings above. Do not
write a launch command here or in the log.

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
outline's fallbacks allow), together meeting the acceptance criteria above, each having passed its review gate and,
where it changes what the app draws, its visual review, and the last removing the TODO.md entry this plan covers. Open,
not merged. If a `## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per
`plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the queue script, write
a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.

## Release note

No release note needed.
