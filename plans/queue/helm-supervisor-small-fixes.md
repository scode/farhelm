# Execute six triage outcomes: small helm, supervisor, setup and installer fixes

Written against main at d3b28820 on 2026-10-11. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan has no dependency on another plan. The `ui-layout-and-escaping-fixes` plan also touches template validation
(the session-name field's hidden characters), in `check_template_shape` and the UI rather than in the helm's store, so
the two may run at the same time; if both are in flight, expect a mechanical rebase around
`crates/farhelm-proto/src/launcher.rs` at most.

## The goal

Carry out six triage outcomes recorded in root `TRIAGE_OUTCOMES.md`, each outcome `fix code`, in one draft PR. The
planning PR that queued this plan set each one's Execution field to
`` planned in `plans/queue/helm-supervisor-small-fixes.md` ``; that is the state to expect at the start, not `pending`.
Their headings, which are also the feedback file names under `review_feedback_queue/`:

- `uninstall-race-first-time-service-setup.md`
- `cancelled-lock-waiters-leave-permanent-registry-entries.md`
- `shutdown-leaves-retained-supervisor-clients-active.md`
- `template-discovery-fails-permanently-once-reply-exceeds.md`
- `pi-getter-serialization-exceptions-escaping-silent-hook-boundary.md`
- `automatic-updates-overwrite-newer-manual-installation.md`

What each one is about, for someone running Farhelm:

- On a fresh Linux machine, running `farhelm helm setup` and `farhelm uninstall` at the same time can leave systemd
  services pointing at a `farhelm` program uninstall has just deleted, while uninstall reports success. SPEC.md
  ("Concurrent and interrupted runs") requires overlapping setup and uninstall to end correct, or refuse.
- The supervisor keeps a small per-key lock table. A request cancelled while waiting for a lock can leave its entry
  behind (about 100 bytes) until the supervisor restarts. No user effect; a cheap cleanup.
- When the helm shuts down, it aborts each host's connection task but does not explicitly retire the host's published
  client the way removing a host does. Nothing in a shipped configuration survives long enough for this to matter (the
  standalone helm exits, the desktop app destroys its runtime), so this is consistency with the method's own contract.
- Agents list launch templates in one reply frame, capped at 8 MiB, and nothing caps the catalog's total size. About 130
  near-maximum templates would break template listing for agents on every host until some are deleted, and since agents
  on any host can save templates, one host's agent could do that to all of them.
- Pi's conversation reporter reads Pi's session getters before its error guard. If one threw after a conversation
  switch, the report would be lost and Resume would stay on the earlier conversation. No such vendor behavior has been
  observed.
- The desktop app's automatic update decides what to install before taking the installer's lock. If the user installs a
  newer build (an RC or a dev build) by hand in that window, the automatic installer can then put the older stable
  release back, and the next launch runs it. Rerunning the installer repairs it.

None of these was reproduced at runtime; all were confirmed by reading the code. Each ledger entry (Assessment,
Decision, Completion criteria) is the authoritative statement of what the PR must do for that outcome. Read the six
entries and feedback files before starting. Where this file and a ledger entry seem to disagree, the ledger entry wins
and the disagreement is a DECISION to log (or a gate trip, per Complexity gate).

Acceptance criteria:

- One draft PR carries out every outcome that did not trip its complexity gate, each meeting its ledger entry's
  completion criteria.
- Every changed or new test has a docstring that says what it proves and why it matters, per root `AGENTS.md` and
  `.agents/test-authoring.md`.
- The PR removes each carried-out outcome's feedback file and index line, updates every outcome's `TRIAGE_OUTCOMES.md`
  Execution field, and passed the review gate.
- The checks under Validation passed, or are reported as skipped with the reason.
- The PR is not marked ready, and nothing is merged.

## Requirement sources

**The user's triage decisions (2026-10-11, recorded in `TRIAGE_OUTCOMES.md`, landed in #1919):** each entry quotes the
user's words. They differ in what a gate trip means, and that difference is binding:

- `uninstall-race-first-time-service-setup.md`: triaged without discussion as straightforward, because the user asked
  that findings "whose outcome is straightforward and needs no significant complexity or redesign" be triaged directly.
  A gate trip (the fix needs a redesign of the lock layout) goes back to the user.
- `cancelled-lock-waiters-leave-permanent-registry-entries.md` and
  `shutdown-leaves-retained-supervisor-clients-active.md`: the user said "fix SUPER cheap ones with complexity gate".
  Only a few lines; otherwise it is discarded during execution (see Complexity gate).
- `template-discovery-fails-permanently-once-reply-exceeds.md`: same "fix SUPER cheap ones with complexity gate", plus
  "anything that means a security boundary is broken like helm being corrupted, or cross-host attackes, must not be
  silently discarded." This one is cross-host, so if the cap is not cheap it goes back to the user, never to `discard`.
- `pi-getter-serialization-exceptions-escaping-silent-hook-boundary.md`: for harnesses other than Claude Code and Codex
  the user said "fix simple ones with complexity gate, if there is a known issue likely to matter to a typical user
  ensure the website's section on that specific agent calls out the caveat." A gate trip goes back to the user. This
  issue is unobserved, so it calls for no website caveat.
- `automatic-updates-overwrite-newer-manual-installation.md`: the user chose "Accept, fix cheap ones": fix it only if a
  guard is cheap; otherwise it is discarded during execution (see Complexity gate).

**The user's plan-time decisions (2026-10-11):** "use the planning system to start planning all of the already triaged
entries"; asked how to group them, the user chose "8 plans as proposed", of which this is the helm, supervisor and
installer group. Review gate: "gpt-6.1-sol high", a single fresh-context agent with the general charter, no swarm. PR
shape: "One PR per plan", so this plan's outcomes share one PR, overriding root `AGENTS.md`'s one-PR-per-outcome rule
for these outcomes only. No-workhorse mode, which `plans/AGENTS.md` requires.

**Planner proposals** are the mechanisms and test shapes in the outline below, checked against main at d3b28820.
Handling a gate trip by dropping that one outcome from the PR and shipping the rest follows the precedent of the earlier
grouped triage plans (#1901).

**Binding repository constraints:** root `AGENTS.md` (Conventional Commits; Finishing work; Execute triage outcomes, as
overridden above; Releases and the changelog; Reproducing failures: narrow tests first; Sharing the machine with other
agents; Agent scratch space; The live install is off-limits; Harness-specific code; the rule against tests that modify
the test process's own environment variables), `plans/AGENTS.md` (Executing one plan), `review_feedback_queue/AGENTS.md`
(Lifecycle), `.agents/test-authoring.md` for any test change, `.agents/narrow-tests.md` for focused runs,
`docs/test-run-evidence.md` for the recorder and the pinned nextest and tmux setup, and `docs/test-sleep-check.md` for
any delay a test adds.

## Outline

Line numbers drift; find the code by name. Verified on main at d3b28820. The six fixes touch unrelated code and do not
interact.

- Setup racing uninstall (`uninstall-race-first-time-service-setup.md`): `farhelm uninstall` takes its locks in
  `lock_and_recheck` (`crates/farhelm/src/uninstall.rs`): the installer's lock directory for a flat installation, and
  setup's unit-directory flock through `setup::lock_for_selected_services` (`crates/farhelm/src/setup.rs`). That
  function returns `None` when no unit directory exists and nothing is selected, deliberately, so a CLI-only uninstall
  does not create `~/.config/systemd/user`. Setup's `install` takes only `lock_unit_directory(unit_dir)`, which creates
  the directory. So a first-ever setup that starts after uninstall's re-check can publish units naming the executable
  uninstall then deletes. The fix must make the two exclude each other in that case, or make one refuse, with a correct
  outcome either way. Candidate shapes, in order of preference; pick the smallest that holds and log it as a DECISION:
  - setup also refuses while uninstall holds the installer's lock directory (`.farhelm-install.lock`) beside the
    executable it is about to name, checked after taking its own flock and before writing, with uninstall re-checking
    for unit files after its own work is done only if that check alone leaves a window. Do not test bare existence:
    reuse the shape reading and refusal wording `InstallLock` in `crates/farhelm/src/uninstall/locks.rs` already has, so
    a lock left by a crashed run is told apart from a held one and the refusal names the remedy;
  - uninstall takes setup's flock whenever a manager is reachable, accepting the empty unit directory it creates, if
    that is judged acceptable under SPEC.md's uninstall contract (it is a new leftover, so check SPEC.md first);
  - setup re-verifies, under its flock and after publishing, that the executable it named still exists, and removes what
    it published if not.

  Whatever the shape, an operation that is refused says so plainly and changes nothing (SPEC.md "Concurrent and
  interrupted runs" accepts refusing). Add a focused unit test beside the existing setup and uninstall tests if one fits
  without new machinery (the setup tests already build unit-directory fixtures and a fake `UnitManager`). SPEC.md
  ("Uninstall scope and interaction", the paragraph on the locks uninstall takes) describes the Linux layout as "setup's
  unit-directory lock when setup's services or unit directory exist" and says a setup refuses whenever uninstall holds
  setup's lock; updating that sentence to describe the chosen exclusion is part of this fix, not a gate trip. Gate: a
  redesign of the lock layout, or a change to how the installer itself locks, is a trip that goes back to the user.
- Lock table entries left by cancelled waiters (`cancelled-lock-waiters-leave-permanent-registry-entries.md`):
  `KeyedLocks` in `crates/farhelm-supervisor/src/service/core.rs` maps keys to `Weak` per-key mutexes, and a guard's
  `Drop` removes its entry only when it is the last holder. A waiter aborted before it builds its guard (in `claim` or
  `claim_before`) leaves an entry whose `Weak` no longer upgrades. Prune such dead entries in `lock_for` while it holds
  the map lock, for example `locks.retain(|_, entry| entry.strong_count() > 0)` before the lookup, or only when
  inserting a fresh entry. The map is bounded by in-flight operations, so a scan per lookup is cheap; say so in the doc
  comment that already promises pruning. A small unit test that drops a `claim_before` future while another guard holds
  the key, then claims an unrelated key and checks the dead entry is gone, is welcome if it fits the module's existing
  tests. Gate: anything beyond a few lines in `KeyedLocks`; a trip discards it per Complexity gate.
- Helm shutdown without retirement (`shutdown-leaves-retained-supervisor-clients-active.md`):
  `ConnectionManager::shutdown` in `crates/farhelm-helm/src/manager.rs` sets `shut_down` and aborts every drained
  actor's task. `stop_actor` beside it shows the missing step: take `status.client` out of each handle with
  `send_modify` and call `retire_withdrawn` on it, outside the map lock. Collect the withdrawn clients while draining,
  release the lock, then retire them. `shutdown` is also what `Drop` calls and steps over a poisoned mutex; keep both
  properties, and keep `retire` (which is synchronous and idempotent) out of the lock hold. One risk to handle:
  `SupervisorClient::retire` calls `abort_agent_tasks`, which does `lock().expect("agent task list poisoned")`, and a
  panic there while `Drop` runs during an unwind aborts the process. Skip retirement when `std::thread::panicking()`
  (the process is unwinding anyway), or make that poisoned case non-panicking; needing more than that is the gate trip.
  Extend the doc comment to say published clients are retired too. The existing test
  `removing_an_actor_retires_a_retained_published_client` is the model for an optional focused test with a retained
  clone. Gate: anything beyond a few lines; a trip discards it per Complexity gate.
- Template catalog size (`template-discovery-fails-permanently-once-reply-exceeds.md`): every template write goes
  through `store_template` in `crates/farhelm-helm/src/templates.rs` (the GUI's `PUT`, and agents' create and edit),
  which checks the template's own shape (`check_template_shape` in `crates/farhelm-proto/src/launcher.rs`: name at most
  128 bytes, fields JSON at most 64 KiB) and then calls `HelmStore::put_launch_template` or `put_launch_template_if`
  (`crates/farhelm-helm/src/store.rs`). Agents list the whole catalog in one `AgentReply::Templates` (`template_listing`
  in `crates/farhelm-helm/src/agent_requests.rs`), which must fit `farhelm_proto::MAX_FRAME_LEN` (8 MiB). Add a total
  cap, checked and written in the same store call, so two concurrent writers cannot both pass it (the store is one
  SQLite connection behind a mutex, so one call is one serialized unit): sum the stored names and `fields_json` lengths
  of every other row plus the new row, and refuse the write when the total exceeds a constant that leaves clear headroom
  under the frame limit for the reply's own encoding and the per-template host names (for example 4 MiB; pick it with a
  comment saying why it fits). `put_launch_template` is a plain `INSERT` today; do the sum and the write inside its one
  store call, as `put_launch_template_if` does its comparison. Replacing an existing template counts its new size, not
  old plus new. The refusal must reach the GUI and agents as the helm's typed invalid-request refusal with a plain
  message (for example "the template catalog is full; delete a template first"), the same way `store_template` maps a
  shape refusal today, not as an internal error. Add a test beside
  `a_template_write_with_a_bad_name_or_field_is_refused` that fills the catalog near the cap and checks the next write
  is refused and nothing is stored, plus that a replace of an existing template under the cap still succeeds. Add one
  sentence to SPEC_impl.md where it describes the template shape check (the paragraph with "a non-empty name of at most
  128 bytes") naming the total cap and why it exists. Also check whether SPEC.md's launch-template section and the
  website's templates page (under `website/src/content/docs/`, per `website/AGENTS.md` and `website/EDITORIAL_RULES.md`)
  should mention the new refusal, and add a sentence where a user would otherwise be surprised by it; log the decision
  either way. Gate: a cap that needs more than this (a schema change, pagination of the agent listing, changes to the
  reply format) is a trip that goes back to the user, never a `discard`.
- Pi reporter guard (`pi-getter-serialization-exceptions-escaping-silent-hook-boundary.md`):
  `crates/farhelm-supervisor/assets/pi-conversation-v1.ts`, `report`, calls `getSessionId`, `getSessionFile`,
  `isPersisted`, `existsSync` and `JSON.stringify` before any `try`. Move them inside the queued step's guard, as the
  OMP reporter (`omp-conversation-v1.ts`) does, so a throw drops this one report silently without breaking the
  serialized chain or the hook. Keep the existing ordering fence (the session id captured at event time and compared
  inside the queued step); the session id read can stay outside the queued step only if it is inside its own `try` that
  returns early, as OMP's `isEligibleContext` is. Keep the two-second child timer exactly as published (the doc comment
  on `PI_ASSET` in `crates/farhelm-supervisor/src/pi_extension.rs` explains why). Changing the asset's bytes requires a
  new published identity: `materialize_asset` refuses a file whose bytes differ under an old name, which once silently
  stopped Pi's reporter on every host that had published the old bytes. So, as that module's spec test demands, give
  `PI_ASSET` a new `file_name` (`farhelm-conversation-v3.ts`), update its row in
  `published_assets_are_pinned_to_their_names` with the new name and SHA-256, update every test in that file that names
  the Pi path, and grep for any other reference to the Pi file name (SPEC_impl.md, `agent_kind/mod.rs` docs, the
  supervisor store). Do not rename the source file. The asset-renaming steps are part of this fix, not a gate trip; they
  are mechanical and the pin test enforces them. Pi has no JS execution harness; do not build one. Gate: anything beyond
  the guard move and the identity bump (a change to the hook, the report format, or a new test harness) goes back to the
  user.
- Automatic update over a newer manual install (`automatic-updates-overwrite-newer-manual-installation.md`): in
  `crates/farhelm-ui/src/desktop/updater.rs`, `check_and_install` re-reads the Installed record after the probe and
  compares it with the latest release, but then downloads and runs the release's own installer (`run_installer`,
  `spawn_installer`), which takes the app-bundle lock (`.farhelm-app.lock` beside `Farhelm.app`) and, in its update
  branch, reads `previous_installed` from `Contents/Versions/installed` under that lock (`scripts/install.sh`) without
  comparing it to anything. A cheap guard is an installer input the updater sets: the Installed record exactly as it
  just read it (`installed` in `check_and_install`), passed in a `FARHELM_INSTALL_*` variable set after
  `farhelm_variables` strips the inherited ones, next to `VERSION_PIN_ENV`. Do not pass `baseline`, which falls back to
  the running version when there is no Installed record; when there is none, omit the variable, or a missing record
  would block every update. Under the bundle lock, when that variable is set and the Installed record no longer names
  it, the installer changes nothing, says why, and exits; the updater then re-reads Installed as it already does. Having
  it report the outcome as idle or up to date rather than a failure when the record names something newer is a nicety:
  if the gate is tight, it is the first thing to drop, and a "did not finish" message in that rare case is acceptable. A
  manual install never sets the variable, so manual version pinning (installing an older release on purpose) is
  untouched. Because the updater always runs the target release's own installer, the guard takes effect for updates to
  the first release that carries it; say so in the changelog fragment's caveat only if it matters to a reader. Cover it
  with a case in `scripts/test-install-sh.sh` (update with the variable set while Installed names a different version:
  nothing changes, exit as designed) and a unit test of the updater's handling of the new outcome if one fits the
  existing updater tests. Before starting, read TODO.md's "Make the Mac installer race-free" entry, which plans a
  redesign covering this race among others: if that redesign has landed by the time you run, this outcome is already
  fixed (record it so, per Complexity gate); otherwise the cheap guard is still wanted now. Older desktop apps will keep
  setting this variable when they run a newer release's installer, so say in the installer's comment beside the check,
  and in a note added to that TODO entry's plan if one exists by then (otherwise in the plan's report), that the
  redesign must accept or harmlessly ignore it. Do not edit TODO.md for this. Gate: if the guard needs more than the
  variable, one check under the existing lock, a test case and a few lines in the updater (for example a new lock, a
  change to the flat layout, or reworking the update order), discard it per Complexity gate and say why.
- Changelog: three outcomes change what a user can see (setup and uninstall no longer race, a full template catalog is
  refused with a message instead of breaking agent listing, an automatic update no longer reverts a newer manual
  install), so the PR's type is `fix:` and it adds a changelog fragment under `releasing/changelog.d/` for each of them
  that is carried out (kind `fixed`), written for someone running Farhelm per `releasing/EDITORIAL_GUIDANCE.md`. The
  other three need none. If gate trips leave only outcomes with no user-visible effect, the type becomes `refactor:` or
  `fix:` as root `AGENTS.md` (Conventional Commits) decides, and no fragment is added.

The complexity gate for each outcome is its bullet's Gate line above, together with its ledger entry.

### Validation

Follow root `AGENTS.md` "Finishing work" and "Reproducing failures: narrow tests first"; no full battery. Choose by what
actually changed:

- `cargo fmt --all -- --check`; `cargo clippy --all-targets -- -D warnings`;
  `cargo clippy -p farhelm --bins -- -D warnings`.
- Focused nextest runs through `scripts/record-test-run.py` (`--runner nextest`, `--kind development`, the pinned
  nextest first on PATH per `docs/test-run-evidence.md`, `--concurrency '4 nextest slots; retries 0'`, a selection
  naming exactly what runs), per crate touched: `-p farhelm` for the setup and uninstall unit tests (`--tmux none`);
  `-p farhelm-supervisor` for the `KeyedLocks` tests and the `pi_extension` tests (`--tmux required` with the pinned
  tmux if the selected module needs tmux, else `none`); `-p farhelm-helm` for the manager's retirement tests and the
  template tests in `templates.rs` and `store.rs`. Read the retained output for runtime `SKIPPED` lines; a skipped
  changed test is not a pass.
- If `scripts/install.sh` changed:
  `sh -n scripts/install.sh && shellcheck scripts/install.sh scripts/test-install-sh.sh` and
  `bash scripts/test-install-sh.sh`.
- If the updater changed: `cargo check -p farhelm-ui --features desktop` and the focused desktop nextest run of the
  updater's tests (`cargo nextest run -p farhelm-ui --features desktop -E 'test(updater::)'` through the recorder,
  `--tmux none`). Both need the webkit2gtk/gtk development packages; if they are missing, report the check as skipped
  with that reason rather than installing system packages.
- The macOS uninstall acceptance (`scripts/test-uninstall.py`) cannot run on Linux; if uninstall code changed, say in
  the report that it was not run and why. Do not dispatch the hosted macOS suite unless a change touches macOS-only
  uninstall code.
- `python3 releasing/check-changelog.py format` for the fragments, and `dprint check` on the changed Markdown
  (`TRIAGE_OUTCOMES.md`, `review_feedback_queue/INDEX.md`, SPEC_impl.md, the fragments).
- If a test adds a delay, `python -B scripts/check-test-sleeps.py` with the isolated interpreter from
  `docs/test-sleep-check.md`.

No browser, e2e battery, provisioning, or website runs: nothing they cover changes. Never run the installer, setup or
uninstall against the machine's own Farhelm installation (root `AGENTS.md`, The live install is off-limits); the test
scripts use private fixture homes.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-helm-supervisor-small-fixes-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root
`AGENTS.md` describes, not in the checkout.

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
  `plan/helm-supervisor-small-fixes/01-small-fixes`.
- All of this plan's outcomes go in one commit, one bookmark and one draft PR, per the user's grouping decision. Within
  this run, if the PR needs correcting, restructure it rather than stacking a correction on top; that applies to a PR an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, the type reflecting the user-visible effect (see Changelog in
  the outline). Changelog fragments follow root `AGENTS.md` (Releases and the changelog), validated with
  `python3 releasing/check-changelog.py format`.
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

Every outcome in this plan carries the user's complexity gate, stated in its ledger entry and in its outline bullet. If
one turns out to need significantly more than the outline describes, do not grow it and do not block the whole plan for
it. What happens next depends on the outcome, as the user decided:

- `cancelled-lock-waiters-leave-permanent-registry-entries.md`, `shutdown-leaves-retained-supervisor-clients-active.md`
  and `automatic-updates-overwrite-newer-manual-installation.md`: never rewrite the recorded Outcome. Set its
  `TRIAGE_OUTCOMES.md` Execution field to "`complete`: gate tripped (<reason>); discarded per the recorded decision",
  followed by the jj change ID, bookmark and PR URL, and remove its feedback file and index line in this PR.
- `uninstall-race-first-time-service-setup.md`, `template-discovery-fails-permanently-once-reply-exceeds.md` and
  `pi-getter-serialization-exceptions-escaping-silent-hook-boundary.md`: leave that outcome out of the PR, keep its
  feedback file and index line, never rewrite its recorded Outcome, and in the same PR set its `TRIAGE_OUTCOMES.md`
  Execution field to `pending`, waiting on the user, followed by what you found and the realistic options with a
  recommendation (see `installer-startup-prune.md` in the ledger for the shape). Name it among the open questions in the
  plan's report. The template cap in particular is never turned into a discard.

The rest of the PR ships. Block per Unattended fallback only when every outcome in this plan was dropped or discarded,
since then there is no fix to deliver; even then, if the discards alone make a PR, deliver that. Code that turned out to
be already fixed on main is not a gate trip: keep the recorded Outcome, set Execution to `complete` with "already fixed"
and the fixing commit, per root `AGENTS.md`, and remove its queue item.

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
the user's decisions. If the PR changes tests, include the full text of `.agents/test-authoring.md` verbatim, as root
`AGENTS.md` requires. Also ask the reviewer to check specifically: that setup and uninstall can no longer both succeed
in the overlap the finding describes, and that a refused run changes nothing; that the lock-table pruning can never
remove an entry a live waiter or holder still references (two different mutexes for one key would let two creates run at
once); that shutdown still never panics on a poisoned lock and retires outside the lock; that the template cap is
enforced inside the write transaction, counts a replace correctly, and reaches the GUI and agents as an invalid-request
refusal; that the Pi asset's new bytes keep the ordering fence and the published timer, and that its identity bump and
pin row are consistent; and that the installer guard leaves manual installs (including installing an older release on
purpose) unchanged. Where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch
command here or in the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Before implementing a substantial departure from the outline above (examples, not a blacklist: a new lock file or lock
protocol, a schema change, a new test harness, edits outside the files the outline names beyond tests, specs and
bookkeeping), and whenever the same component has needed repeated corrective review rounds, run a fresh-context review
through galaxy-brain with this charter, supplying the ledger entries, the user's decisions above, this outline, the
current diff and the proposed departure (what changed, why it is necessary, and which simpler alternative was ruled
out):

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
alternatives considered: in particular which exclusion shape setup and uninstall got and why the others were rejected,
where the lock-table pruning runs, the template cap's value and how it is counted, the Pi asset's new identity, the
installer variable's name and the updater's handling of a superseded install, whether TODO.md's race-free installer work
had landed, the Conventional Commit type and the fragments written, every outcome dropped or discarded by its gate and
why, every runtime `SKIPPED` seen, every check skipped and why, and every review finding you decided not to follow. The
user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. Remove planner-invented machinery that turns
out unnecessary when the ledger's requirements still hold. A material scope expansion, a weakened guarantee, or an
omitted required behavior needs an agreed fallback or the user's decision; a review finding or a log entry is not
authorization. For a single outcome, the agreed fallback is the complexity gate above. If current code or specs have
moved so that a recorded decision no longer applies, treat it like a gate trip for that outcome, per root `AGENTS.md`
(Execute triage outcomes); never re-triage an item yourself. Block per `plans/AGENTS.md` (Executing one plan, step 10)
only when no outcome is left to ship or the shared PR itself cannot proceed without the user.

You never ask the maintainer anything in conversation while running this plan: not a direct question, not an optional
one, not an offer of more work, and not a turn that ends waiting for a confirmation. Every fork is either decided by you
and logged as a DECISION, or, when only the maintainer can decide it, a block per `plans/AGENTS.md` (Executing one plan,
step 10). Things the maintainer should know and possible follow-ups go into the plan's report, written as statements.

## Done criterion

The plan is complete when its one draft PR exists, carries out every outcome of this plan that did not trip its
complexity gate (and records the discards and returns the gate called for), meets the acceptance criteria above, and
passed the review gate, with every outcome's `TRIAGE_OUTCOMES.md` Execution field updated and the queue items of the
carried-out and discarded outcomes removed. Open, not merged. If a `## Decisions` section exists, its latest entry must
also be satisfied. Then close the plan per `plans/AGENTS.md` (Executing one plan, steps 11 and 12): deliver its report
through the queue script, write a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.
