# Brick test a release

This file is the whole brick-test procedure, and the place the root flow index sends a **`brick test release vX.Y.Z`**
request. "Release brick tests" holds the rules an agent running the test follows from its first action (it used to be a
section of the root `AGENTS.md`); the sections after it are the procedure those rules refer to. Read the whole file
before inspecting Tart state.

> NOTE: In an unattended run (plan executor, landing monitor, delegate), an instruction here to ask, confirm with, or
> propose to the maintainer follows `AGENTS.md`, "Unattended runs".

## Release brick tests

When asked **`brick test release vX.Y.Z`**, follow `releasing/mac-vm-test/BRICK-TEST.md`. Discover the private machine
profile there, read its operator policy and journal, and run the checks in priority order, in the order in
`releasing/mac-vm-test/BRICK-TEST.md`, "The test and its scopes", with guest-only product mutations. **P0** asks whether
an install or upgrade bricks Farhelm: fresh install/start, click-to-upgrade discovery/relaunch (stable candidates only),
a separate installer upgrade, the local and the remote supervisor each upgrading, then one successful new agent on each.
**P1** asks whether an upgrading user keeps what they had: on each host, an agent session started on the previous stable
must survive the upgrade in its terminal host, then restart onto a new terminal host and resume its conversation; then
Remove/re-add the remote Linux host with its sessions preserved, and Uninstall after ending all its sessions. A separate
pair tests remote Mac Uninstall and confirms that its saved session survives. **P2** sends one live feedback submission
and checks signed announcements on P0's last Mac guest, before that fixture is exported and cleaned up. Broader work is
bonus coverage after the tiered checks. Preserve failed attempts and repair safe harness or setup problems under "Repair
and continue"; keep working toward the requested brick-test coverage instead of ending the task at the first obstacle. A
P0 failure stops the rest of its pair. A P1 failure stops later P1 checks on the same pair. Neither a P1 nor a P2
failure invalidates P0, blocks cleanup or fences bonus passes; each is reported promptly with its own verdict. A request
to **stop after P0 checks** ends after recording P0 evidence and cleaning up its successful fixtures; no old sessions
are checked, no feedback is sent and no bonus fixture starts. **Stop after P1 checks** ends after P0 and P1 evidence and
the last fixture's cleanup, without the send. **Stop after P2 checks** ends after P0, P1 and P2 and the last fixture's
cleanup. Answer **did P0 checks succeed?** from the separate P0 verdict and seven results, naming anything incomplete,
and **did P1 checks succeed?** and **did P2 checks succeed?** from their own verdicts and named checks. The full verdict
includes P1 and P2; whether their failure blocks promotion is the maintainer's call for that release. Resolve missing
prerequisites from evidence and existing authorization; ask only when a needed binding, permission or consequential
decision cannot be established. A test request does not authorize publication or release promotion.

Use the guest-native executor of `releasing/mac-vm-test/BRICK-TEST.md` for actual tests: an independent Codex process
inside the Mac guest drives native CUA and survives Farhelm quit/restart. The host coordinates Tart lifecycle, pinned
networking and evidence, but does not drive product UI. Resolve native permissions and explicit Mac/Linux
credential-copy consent during fixture setup. Prepared bases remain credential-free; standing authorization may cover
this machine's disposable test clones. An OS/CUA/authentication permission block during P0 or an evaluation is a harness
bug: end that attempt, retain the failure, repair already-authorized setup separately and start a safe new attempt or
run. Do not manually approve or switch to host UI to rescue it. Ordinary Farhelm confirmations are authorized product
interactions handled by the guest tester.

For full or stop-after-P2 scope, resolve authorization during setup for the live feedback send to farhelm.io, which
creates an issue in the private inbox. Stop-after-P0 and stop-after-P1 scopes do not authorize or require that send.

Before creating a new test pair, inspect live Tart state and reconcile leftover brick-test guests using their original
ownership manifests and journal. The new test request authorizes clean shutdown of abandoned previous test pairs without
asking again, preserving their disks and evidence. Do not interrupt an active operator, delete retained data, force-stop
guests or touch unrelated VMs. Verify the earlier pair is stopped and its foreground owner has exited. Follow
`releasing/mac-vm-test/BRICK-TEST.md`, "Reconcile guests and prepare coverage", for the startup reconciliation
procedure. Host Tart window binding is needed only during fixture setup; actual product computer use binds apps directly
inside the guest.

Run this gate **before** moving `/latest` to the candidate. It is expected that `/latest` still names the preceding
promoted stable; do not treat that difference as a blocker or ask to promote first. Upgrade baselines are the three
newest stables from the candidate's upgrade floor through that public default, excluding unpromoted intervening tags;
older stables inside the floor are deliberately not tested. The initial floor is v0.23.0; `UPGRADE_FLOOR` in
`brick_test.py` stays equal to the shared Rust constant, checked by the portable unit suite. For an old app that
supports it, quit and relaunch inside the Mac guest with
`open --env FARHELM_DESKTOP_UPDATE_LATEST=vX.Y.Z "$HOME/Applications/Farhelm.app"`, then use the in-app updater. Older
apps use the manual candidate-installer path. Follow the procedure's verification and override removal checks in both
cases.

During fixture setup only, read `releasing/mac-vm-test/OPERATIONS.md`, "Computer use and continuity", before entering
guest text. Update those notes and `releasing/mac-vm-test/BRICK-TEST.md`, "Release brick tests", when a new Tart input
workaround is verified, so the next agent does not repeat the investigation. `typeText` and rapid physical key sequences
can drop characters; per-character `pressKey` calls with `durationMs: 100` delivered unmodified text reliably in the
release test. To replace a field, triple-click with `durationMs: 100`, then inspect a screenshot for the actual
selection highlight before typing. Modifier chords can reach the guest without their modifier (`tl:` became `tl;`, and
select-all became a literal `a`); verify punctuation and the resulting reactive form state before submitting. The
verified workaround is guest-only clipboard preparation through the journaled controller
(`printf %s <text> | /usr/bin/pbcopy` inside the Mac guest), followed by computer use on the guest's Edit menu. Click
Edit, navigate with `Down` keys held for 100 ms until Paste is visibly highlighted, then press Return. Direct clicks on
guest native menu items can dismiss the menu and reach the underlying window instead. Keep Tart's shared clipboard
disabled and the host clipboard untouched; inspect both the pasted text and the form's response before proceeding. If
Tart window lookup fails, follow the reconciliation notes in `releasing/mac-vm-test/OPERATIONS.md`, "Computer use and
continuity": the guests may still be running. Preserve their foreground supervision and resolve GUI access instead of
restarting guests or using the host's Farhelm.

## The test and its scopes

Tell a host agent in this checkout: **`brick test release vX.Y.Z`**. It should follow this document, using the private
machine profile discovered below. The host coordinates lifecycle, planning and retained evidence. An independent Codex
tester inside the Mac guest drives genuine native computer use and published installers; it survives Farhelm's own
quit/restart and launches separate authenticated product agents through Farhelm. It does not publish, promote a release
or modify the host's installed Farhelm.

This is a **pre-promotion gate**: run it after the candidate's version-path artifacts are published and before moving
`/latest` to it. The endpoint is expected to name the preceding promoted stable. That difference is not a missing
prerequisite and does not call for promotion; it selects the override or manual update path below. An after-promotion
run remains useful, but cannot gate the candidate's initial exposure.

The checks come in three named tiers, ordered by what a failure costs a user: whether the release bricks an installation
(P0) matters more than whether an upgrading user keeps their sessions (P1). The feedback and announcement paths are P2.
A release that cannot resume old sessions but still installs, upgrades and starts new ones fails P1, not P0.

| Tier | Checks, in order                                                                                                                                                                                                                                                                                                                | A failed or unavailable check stops                                                    |
| ---- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------- |
| P0   | `p0-fresh`: fresh install and app start. `p0-updater-upgrade` (stable candidates only): click-to-upgrade discovery and relaunch. `p0-installer-upgrade`: installer upgrade and Restart to update, local supervisor upgrade, remote supervisor upgrade, then one genuine new agent on the local host and one on the remote host. | the rest of its own pair's P0 and P1 checks, P2, and bonus work; never another P0 pair |
| P1   | On `p0-installer-upgrade`: start one old session per host, then prove both survive and resume, and exercise remote Linux Remove/re-add and Uninstall. On `p1-remote-mac`: provision a headless Mac peer and prove its Uninstall preserves session data.                                                                         | later P1 checks on the same pair, which are recorded as skipped                        |
| P2   | On `p0-installer-upgrade`: one live feedback send (`p2-feedback-send`), then signed announcements (`p2-announcements`).                                                                                                                                                                                                         | nothing                                                                                |

The seven named P0 results are fresh install, click-to-upgrade, installer upgrade, the two supervisor upgrades and the
two new agents. Each supervisor's upgrade is observed on its own, before any new agent runs on it, so a flaky agent turn
cannot obscure whether a supervisor came back. The P0 pairs do not depend on each other's results: once a failed pair is
released (with `--discard-failed` after the maintainer decides its retention, also for a pair stopped by an automation
failure), the next P0 pair still runs, so a broken updater cannot hide a bricked supervisor. Pause dependent checks
while repairing a failure or unavailable prerequisite under "Repair and continue". Broader functionality and
historical-version coverage are bonus work after P0, P1 and P2, never prerequisites inserted between them.

`old-sessions` has to come before the relaunch but runs on the previous release, so nothing it sees is a candidate
failure: a fail reported there is kept as could not run, and it stops only P1 on that pair. The five named P1 results
are the two old sessions, Linux Remove/re-add and Uninstall, and Mac Uninstall. Remove must leave the remote host
exactly as it was. Uninstall comes after ending all remote sessions and must leave no Farhelm process, unit or binary
behind while keeping the data. A guest harness failure at a P1 check makes that check could not run (the failed attempt
stays retained) instead of stopping the pair; a permission block still stops it.

P2 runs after every runnable P0 check passed with retained evidence, on the Mac guest of `p0-installer-upgrade`, before
the final export and cleanup of that pair. Both checks wait for the installer pair's P1 results to be recorded or
skipped. The separate Mac P1 pass starts after the installer pair has been cleaned up. If any runnable P0 check did not
pass, the P2 checks are recorded as skipped. The feedback's first line is `[brick-test] vX.Y.Z`, naming the candidate,
so the private inbox issue is titled `Feedback: [brick-test] vX.Y.Z`. Report `p2.verdict` and both named results
(`feedback-send`, `announcements`) separately from P0 and P1. A P2 failure or unavailable check is reported promptly and
does not stop bonus coverage. The full `verdict` includes P1 and P2.

The updater only discovers stable releases, so an RC or dev candidate plans no `p0-updater-upgrade` pair. Its installer
pair carries the supervisor checks either way. Run the installer's in-scope P1 and P2 checks before its cleanup. Then
run the separate Mac P1 pair and bonus coverage as scope permits. The formal P0 verdict still requires all seven
results, so it stays could not run. A stable release requires discovery: if the old app cannot discover the stable
candidate, record a failed discovery requirement and stop that pair. An observed updater failure or missing evidence
stops dependent work for every release kind.

The coordinator reports `p0.verdict` and the seven named `p0.checks` separately from `p1`, `p2` and the full `verdict`.
**Did P0 checks succeed?** means all seven ran and passed with retained evidence. For an RC or dev build, say upfront
that in-app update discovery was skipped as expected because the updater only discovers stable releases. Use the
explanation in `p0.unavailable_checks` and include the installer upgrade and Restart to update outcome. When
`p0.summary` says all runnable checks passed, lead with that. Otherwise name any checks that stopped and why. Give P1,
P2 and bonus blockers their own plain-language causes.

**Stop after P0 checks** means record that scope with `init vX.Y.Z --stop-after-p0`, complete P0, export its evidence,
clean up its successful fixtures, and end the session before P1, P2 or bonus work; no old sessions are started and no
feedback is sent. If requested during a run, persist the scope with `brick_test.py stop-after-p0 "$test_run"`, finish
only the remaining P0 work and cleanup, and retain the request in the private working log. If later work already began,
stop that work and preserve its partial evidence; the scope command fences future checkpoints and fixtures but does not
itself cancel GUI actions or shut down guests. A P0 pass does not claim that P1, P2, bonus checks or every historical
upgrade ran.

**Stop after P1 checks** (`init vX.Y.Z --stop-after-p1`, or `brick_test.py stop-after-p1 "$test_run"` mid-run) finishes
P0 and P1, including the separate Mac pair when planned, and cleans up the last pair before stopping. It needs no
feedback approval. **Stop after P2 checks** (`--stop-after-p2`, or `stop-after-p2`) also runs both checks on the
installer pair before the Mac pair. It stops before bonus fixtures. Like full scope, it needs feedback approval in its
preflight, and the mid-run command rechecks it. A run started with a narrower scope had no approval for the live
feedback send, so obtain that approval before widening it to P2. `requested_verdict` combines the tiers the scope
includes. **Did P1 checks succeed?** and **did P2 checks succeed?** are answered from `p1.verdict` and `p2.verdict` and
their named checks, never inferred from P0 or the full verdict.

If a checkpoint was already dispatched when scope narrowed past its tier, it finishes: in particular, the send cannot be
recalled. Retain its actual result, marked `dispatched_before_scope_stop`, and report that outcome separately. The stop
request prevents later checkpoints of the excluded tiers; it never erases an in-flight observation. An initial P0-only
or P1-only run never dispatches P2.

Widening a run whose installer pair was already cleaned cannot recover its P1 or P2 checks. Start a new authorized run
to recover that coverage. A planned, untouched remote Mac pair can still run once P1 is in scope and all P0 pairs are
cleaned up. Widening while the installer pair is still live cannot start `old-sessions` once that pair's upgrade ran
either: the baseline has to come from the previous release, so a checkpoint whose place in the pair's order has passed
never runs, and the old-session checks stay could not run.

When the maintainer resumes full coverage after a temporary scope stop, use `brick_test.py continue-full "$test_run"`.
Resolve live-feedback authorization first. This rechecks setup consent without clearing observations or retrying sends;
it cannot recover checks on a pair already deleted.

A run planned before this P0/P1/P2 layout (its P0 held the old sessions and remote menu, and feedback was P1) keeps its
saved report under the questions it was planned with, but this coordinator refuses to prepare, test, record, rescope,
reconcile or clean it. Finish it with the coordinator checkout that started it, or start a new run.

Read [RECIPE.md](RECIPE.md) for the product checks and [OPERATIONS.md](OPERATIONS.md) for guest commands, GUI mechanics
and evidence collection. [BRING-UP.md](BRING-UP.md) covers preparing another machine. A previously successful run proves
that run's versions and paths; a new candidate needs its own checks against its actual artifacts.

## Repair and continue

### Requests to change the process

An explicit request to change how brick testing works in general includes implementing and validating the change and
opening a draft PR. Merge it when the maintainer asks. Use the repository's PR workflow and carry only the intended
changes into the PR.

If a request to unblock or redirect a run would change the process and could mean either a permanent change or a one-off
exception, ask the maintainer which they intend before changing the process. Follow the repository's unattended rules
when nobody is watching. A clearly scoped one-off needs no clarification. Keep any temporary code or instruction edits
in an isolated checkout or otherwise track them separately from existing edits. After the run or attempt ends, remove
those temporary edits and verify that the working copy has no changes from the exception. Retain the run's evidence and
record the exception in its private log.

The goal is to complete the requested brick test. A recoverable harness problem is work to diagnose and fix, not the
default endpoint of the task. Inspect its actual error, retained events, current guest state and journal; make a narrow
repair or evidence-preserving workaround, validate it, and continue without asking for routine implementation choices.
Patch the shared harness or these instructions when the cause can recur. Preserve unrelated checkout changes.

A tool failure can end one attempt without ending the task. Keep the original attempt, hashes and observation; never
overwrite accepted results, clear a failure flag by hand, waive a checkpoint, fabricate a receipt or replay an action
whose outcome is uncertain. Use a supported retry transition only when retained evidence proves its safety. Otherwise
repair setup separately and use a new disposable fixture/run. Keep each failed run in the report alongside later
observations. Try at most three evidence-based repairs for the same cause; stop repeating a workaround that makes no
progress and report the remaining concrete block.

For a read-only prerequisite response with `status: not-ready`, the guest tester follows its corrective instruction and
samples again after readiness. No acceptance receipt exists until the sample succeeds; the bridge bounds repeated
deferrals. Older runs stopped by the early new-agent sample guard can use
`brick_test.py retry-readiness "$test_run" <pass>`, then `test` again. The command requires acknowledged termination,
intact earlier evidence and retained events proving that no product action or uncertain call occurred. It archives the
failed claim and its hashes in `readiness_retries`; the next checkpoint gets a fresh attempt. It never retries feedback
or clears a real product failure. If it refuses, inspect why and use a clean run where necessary, not a state-file edit.

For a diagnosed fixture prerequisite recorded as `could-not-run`, repair it and inspect the native event sequence for
uncertain or already-completed actions. When repeating the checkpoint is safe, use
`brick_test.py retry-unavailable "$test_run" <pass> --reason '<verified repair and replay-safety evidence>'`, then
`test`. This retains the original observation, its hashes and later skips caused by that observation in
`checkpoint_retries`. Earlier slots and skips caused by other checkpoints stay settled. Later executed checkpoints and
older skips without a recorded cause prevent retry, as do product failures, automation errors, uncertain tool calls, P1
and P2. Record what the host verified before retrying. If a product action has an uncertain outcome, use a new fixture
instead.

Continue feasible independent diagnosis and checks while repairing a blocker; preserve the declared P0 and P1
dependencies and do not send P2 until P0 passes. A confirmed product defect remains a failure even if another scenario
works. Do not replace published release bytes with a source build to manufacture a release pass; diagnose the defect and
report the coverage that can still be established.

Stop or ask when proceeding needs missing authorization, risks the live install or unrelated state, could destroy
retained evidence, or has a consequential outcome that cannot be established safely. Permission prompts still end the
current attempt; no host UI rescue or unapproved permission/authentication change. Repair authorized fixture setup
separately. Publication, promotion, frozen-base changes and deletion retain their existing fences. A final could-not-run
report should identify the unresolved prerequisite or risk and the repairs attempted, not merely repeat a tool error.

## Discover the private machine

`brick_test.py` reads the explicit `--profile` argument, then `FARHELM_BRICK_TEST_PROFILE`, then
`~/.config/farhelm/brick-test.json`. A missing named override fails rather than falling back. The profile must be mode
0600 and outside the public checkout. The required format-two fields are shown with fictional bindings below. An
optional `remote_mac_base` is described after the example.

```json
{
  "format": 2,
  "mac_base": { "name": "mac-test-base", "mac_address": "02:00:00:00:00:01" },
  "linux_base": { "name": "linux-test-base", "mac_address": "02:00:00:00:00:02" },
  "linux_user": "guest",
  "tart_home": "~/.tart",
  "journal": "operator/LOG.md",
  "evidence_root": "~/.tart/farhelm-evidence",
  "operator_policy": "operator/AGENTS.md",
  "resource_limits": { "minimum_free_gib": 100, "maximum_run_growth_gib": 60 },
  "route_via_gateway": false,
  "agents": ["codex"],
  "guest_tester": {
    "settings": {
      "codex": "/opt/homebrew/bin/codex",
      "python": "/opt/homebrew/bin/python3",
      "desktop_app": "/Applications/Codex.app",
      "model": "gpt-6.2-sol",
      "effort": "high"
    },
    "auth_source": "~/.codex/auth.json",
    "authorization": "operator/credential-authorization.json",
    "fixture_receipt": "operator/native-fixture.json"
  }
}
```

Relative paths belong to the profile's directory, not the checkout. `~/` means the current user's home; shell variables
are unsupported. Read `operator_policy` and the journal before acting. They define local authorization, logging and
failure retention. The profile holds bindings and references, never credential material. Its exact normalized bindings
are copied into private run state; changing the profile cannot redirect an existing run's cleanup.

Use `gpt-6.2-sol` as the Codex baseline. Check the installed private profile before starting a run and update an old
model pin during setup. The same model drives the guest tester and the Codex sessions it launches through Farhelm. Older
apps may need the launcher’s custom model field. The fixture receipt binds the prepared bases and local runtime paths.
Each new run exercises its configured model and effort during native preflight.

VM storage (`tart_home/vms`) and evidence must share a filesystem for the single-volume disk budget. The profile loader
and the next-pair preparation both check that invariant, including mounted or symlinked storage. Budget observations
measure the actual VM-storage volume. A split-volume setup needs an explicitly supported accounting scheme; this
coordinator refuses it rather than checking a spacious evidence disk while the VM disk fills.

Format-one profiles support command-only fixture smoke and historical status and cleanup. New release tests require
prepared native fixtures and format two. Set the optional `remote_mac_base` using the same name/MAC fields as `mac_base`
if the headless peer needs a different image. The peer defaults to `mac_base`. Its image needs Remote Login, Homebrew
Python at `/opt/homebrew/bin/python3` for SSH setup and Python at `guest_tester.settings.python` for the checks. One
interpreter suffices when those paths match. All reusable bases must be free of Codex authentication and Farhelm state.
The private control project owns the bases, credential authorization, image provenance, budgets, journal and profile
installation. It may have its own setup adapter. Farhelm does not discover that project by scanning home directories or
depend on its location. Its portable interface is this profile. Installation of the profile is a separate host write to
agree during machine setup. If the profile, approved bases, GUI access or permissions are missing, report **could not
run** and resolve that prerequisite with the maintainer. Do not install host software or alter host networking as a
fallback.

## Plan and start a run

Run commands from the Farhelm checkout with Python 3.11 or newer. Replace the release placeholder with the requested
exact tag. Do not interpret `vX.Y.Z` literally.

Before `plan` or `init`, compare the candidate tag's `UPGRADE_FLOOR` in `crates/farhelm-proto/src/version.rs` with the
running checkout's literal in `brick_test.py`. A candidate predating the Rust constant has the documented v0.23.0 floor.
They must agree: testing an earlier-base candidate with a newer checkout's higher floor would silently omit supported
upgrade sources. If they differ, stop planning and use a coordinator checkout whose floor matches the candidate before
continuing. The portable agreement test checks the two constants within one checkout; it cannot establish this
candidate-tag comparison for the operator.

Use a coordinator that matches the candidate's default remote update policy too. This procedure observes automatic
remote updates on candidate relaunch. For candidates from before that default, use the coordinator from before this
change, which exercises the manual Update action.

### Resolve permissions during fixture setup

During environment readiness and basic coverage planning, inventory the permissions needed for the **requested scope**
before creating a release run. Read the profile, operator policy and previous provisioning/authentication records;
inspect available prerequisites without printing credentials or starting frozen bases. Include the genuine agent CLI and
authentication on **both** Mac and Linux, even though their P0 checks run later. A stale copied login may need a
refresh; a successful `login status` is not proof that it can serve a conversation.

Make any necessary approval request concrete: identify the candidate/run, credential source and each working-clone
destination, CLI installation source, guest GUI permissions, and the bounded private evidence to retain and where it
will go. Review the planned operations first, reuse explicit authorization that already covers them, and ask for the
remaining approvals together. Credential approval must name Mac and Linux separately; authorization for one does not
cover the other. Setup may record explicit standing consent for this machine's disposable Mac and Linux test clones. The
executor checks that private record and injects the named authentication file via stdin with mode 0600 before its first
turn. Keep credentials out of reusable fixtures, logs and exports. Record consent privately without its contents.

For full or stop-after-P2 scope, include approval to send the live P2 feedback through the candidate app to
`farhelm.io`: it creates an issue in the maintainer's private feedback inbox, with the fixed brick-test title and no
contact information. Reuse explicit authorization covering that send; otherwise ask alongside the other missing
approvals. Stop-after-P0 and stop-after-P1 scopes do not include this send and need no permission for it.

Record that separate setup approval as `feedback_send_approved: true` in the private authorization record. It authorizes
only RECIPE's fixed brick-test message, sent through the disposable candidate app to `farhelm.io` and its private inbox,
with blank contact. Credential-copy consent does not authorize this send. Full and stop-after-P2 initialization and
testing refuse before product mutations without that field; widening scope to include P2 and the P2 action boundary
recheck it. Narrower scopes ignore the field and never schedule the send. Do not set it without the maintainer's actual
authorization.

Resolve known permission prerequisites before installing or launching Farhelm. Readiness probes in owned clones may
reveal a missing requirement that the provisioning records could not establish; settle it before product checks rather
than deferring it until the corresponding supervisor/agent step. This is permission planning, not an extra agent
acceptance test before fresh installation. Keep actual setup beside the P0 step it supports and retain P0's order.

An OS/CUA/authentication approval request during an actual test is an automation bug. Stop and retain that attempt;
repair and revalidate the fixture separately, then start a new run. Never rescue an evaluation with host UI or a manual
approval. Ordinary Farhelm confirmations (including its YOLO launch confirmation) are authorized product steps and the
guest tester handles them without changing confirmation preferences. In P0-only scope, avoid bonus prerequisites.

### Reconcile guests and prepare coverage

Before initializing a new run or cloning guests, inspect live `tart list`, the external journal and previous run
manifests under the profile's evidence root. Reconcile any running brick-test pair first. A new brick-test request
authorizes clean guest shutdown of an abandoned previous test pair, with its disks, manifests and evidence preserved; no
additional confirmation is needed for that shutdown. Establish ownership from the original manifest and unchanged VM
identities, and use that run's journaled `control.py shutdown` for each guest. Verify both stopped states and the old
foreground owner's exit before starting the new pair. Do not delete retained clones or evidence, force-stop a guest,
touch frozen bases or unrelated VMs, or interrupt another active operator's test. If ownership or active use is unclear,
resolve it with the maintainer before cloning. A pending evidence-export approval does not require an abandoned guest to
remain running: retain its disk and record where the unexported evidence lives.

This check belongs at startup because ownership and storage accounting still depend on Tart. Product computer use binds
directly to apps inside the guest, so an actual test never needs to locate or type through a host Tart window. Host
window binding is confined to the infrequent fixture setup procedure.

- Confirm release tags are available in the checkout; fetch missing tags from its normal Git remote if authorized.
  `python3 -B releasing/mac-vm-test/brick_test.py plan vX.Y.Z` performs read-only release discovery. It resolves the
  candidate commit, observes `https://get.farhelm.io/latest`, lists eligible stable upgrades from the upgrade floor and
  checks candidate installer metadata availability. Availability and an advertised hash are **not** signature
  verification. Needed payloads and the previous releases' inputs must also exist for their actual guest flows.
- Upgrade baselines run through the stable named by `/latest`, excluding the candidate itself, and cover at most the
  three newest eligible stables (`HISTORICAL_UPGRADE_LIMIT`). A newer intervening tag that has not been promoted is not
  another installed stable baseline just because it exists, and an older stable past the cap is a deliberate omission;
  do not require their artifacts or treat their omission as a coverage gap. The candidate's own published artifacts are
  still required.
- Run `python3 -B releasing/mac-vm-test/brick_test.py init vX.Y.Z`. Save the returned private `run_dir` as `test_run`.
  The output names every pass and required checkpoint. The run gets its own directory under the configured evidence
  root, a profile snapshot, candidate input copies and initial free-space observation. No guests start yet. Add
  `--stop-after-p0`, `--stop-after-p1` or `--stop-after-p2` when requested; each refuses bonus fixtures and records of
  the tiers after it.
- Read the candidate's release notes, changelog fragments and protocol/schema changes. Fill RECIPE's addendum inside
  `test_run/addendum.md` (nonempty, at most 32 KiB, no credentials). Record all planned pairs, Codex selection, omitted
  agents and concrete extra checks. The guest receives this file and the declared checkpoint contracts for bonus work.
  Add each extra obligation on a bonus pass with `brick_test.py add-check "$test_run" <pass> extra-<name>`, adding
  `--gui` for a GUI check. Keep P0's prerequisites minimal; defer historical-source investigation and feature-specific
  fixtures until P0 passes. Missing older inputs are a bonus coverage gap, not a reason to delay the previous-stable P0
  checks.
- In a supervised foreground tool session, run
  `python3 -B releasing/mac-vm-test/brick_test.py serve "$test_run" p0-fresh`. Keep this session alive. The driver
  verifies both frozen bases' identities and stopped states before cloning, starts the owned pair, establishes command
  readiness and pinned guest-to-guest SSH, and stages `~/farhelm-brick-test/continuity.py` in Linux. Wait for its ready
  report. Linux is headless; macOS retains a logged-in GUI session for its own native tester. Use the normal visible Mac
  start for product tests; `--headless` is reserved for command-only setup work.

Use `python3 -B releasing/mac-vm-test/brick_test.py status "$test_run"` to inspect progress and save private
`report.json`. This command is the JSON interface for automation. A run planned under the earlier P0/P1 layout is
refused by every command, `status` included: finish it with the coordinator checkout that started it, or read the
`report.json` that coordinator saved, which stays as it was written. Lifecycle commands can print guest shutdown or
controller messages before their final report; retain that output as diagnostics rather than parsing it as one JSON
document. A deleted pass retains its evidence and verdict but omits live `guests` handles. Read its `phase` and the
named `p0.checks` from `status` instead of expecting those handles after cleanup.

Each prepared pass exposes `guests`, `ssh` and a shell-quoted `control_prefix` for the existing controller. Use those
live handles, not remembered VM names or DHCP addresses. Only one working pair is allowed at a time. The driver checks
the free-space floor and observed volume growth before another pair; these checks are not a continuous disk quota.
Measure free space during large installations too, following operator policy.

## Perform the product checks

Run RECIPE's P0 procedure first: `p0-fresh`, then `p0-updater-upgrade` (stable candidates), then `p0-installer-upgrade`.
The installer pair holds P0 steps 3–5, Linux P1 and P2. The two upgrade fixtures are independent: a successful installer
run cannot prove discovery by the old app's updater, and a failed discovery cannot hide whether the supervisors upgrade.
The coordinator runs the P0 fixtures in their planned order without making one depend on another's result, fences bonus
work on every P0 check passing with valid evidence, stops a pair's remaining P0 and P1 checks after one of its P0 checks
fails or is unavailable, stops the later P1 checks after a P1 one does, holds P2 until every P0 check of the run passed,
and prevents adding extra feature checks to P0. Export and record each successful fixture, then clean it up before
starting the next. Retain the installer pair for its in-scope P1 and P2 checks. When P1 is in scope, start
`p1-remote-mac` once all P0 pairs have been cleaned up, including any failed pair released after a retention decision.
The two Mac guests belong to this one working pair. Never provision frozen bases.

The candidate shows Welcome inline in the empty main pane, so neither its first open after a fresh install nor its
relaunch after an upgrade has a Welcome card to dismiss. A previous release used as an upgrade baseline may still open a
Welcome card over the session UI on its first open: dismiss it with **got it** if it appears, before driving the session
UI. This is ordinary startup, not an additional P0 feature fixture.

For each ready fixture, run `python3 -B releasing/mac-vm-test/brick_test.py test "$test_run" <pass>`. Keep its separate
`serve` owner alive. The `test` command injects already-authorized credentials into the GUI Mac and its Linux peer. For
`p1-remote-mac`, only the GUI Mac receives the login. The test checks actual native screenshot/click/text entry before
Farhelm, then executes the ordered checkpoints inside the owned disposable guest. Signature verification, fixture
cleanliness and fresh installation run as fixed command-only children without model turns. GUI checkpoints use fresh
guest Codex turns with shell tools disabled. Fixed operations and native CUA share one serial failure gate; their
guest-only approval is explicit in the invocation. It selects the update branch from a fresh `/latest` observation at
the action boundary, imports bounded private evidence, and stops at the first model, tool or permission failure, or P0
or bonus product failure. A P1 failure, unavailable observation or harness failure instead makes that check fail or
could not run and skips the pair's later P1 checkpoints (each skip is journaled and printed) before going on to P2; a P1
permission block still stops the pair. A P2 product failure leaves the other P2 check available. A stop ends the
attempt, not the host's repair work under "Repair and continue". The built-in whole-attempt automatic recovery is a
provider HTTP 429 before any test action: strict retained event and protocol records must prove no command, native
action, approval or product observation, and the executor must acknowledge stopped cleanup. Up to three fresh attempts
back off 60, 120 and 240 seconds within the checkpoint deadline; every failed attempt and its hashes remain retained and
reported. No permission block, uncertain action or feedback send is retried. Update checks reobserve the public default
after backoff and stop if it changed. `--timeout` sets the base budget per checkpoint (30–3600 seconds, default 1200).
The effective budget includes any checkpoint-specific floor listed below and is shared by the initial turn, backoff and
retries. Exact native text entry uses paste with reactive-value verification; the tool omits `typeText` after observed
partial delivery. Each turn receives its complete selected checkpoint contract, so recipe-file browsing is not part of a
test. Bonus native readiness additionally proves a real Finder window move and folder double-click before installing
Farhelm. P0 retains its smaller prerequisites. A gesture tool's successful return is insufficient: an independent
same-window frame change must establish drag delivery, or that fixture stops as unavailable. Status remains queryable
while a turn runs; cleanup and another tester refuse an active attempt. If the controller disappears, reconcile the
retained claim and guest process before cleanup; a stale claim is not permission to start a second tester.

If that owner has disappeared, run `brick_test.py reconcile-tester "$test_run" <pass>`. A kernel-held owner lease
refuses interference with a live coordinator. Reconciliation asks the exclusive guest executor to cancel and requires
its stopped acknowledgement, or observes a stopped Mac VM. It retains the failed run and releases only the cleanup
fence. A missing acknowledgement leaves the fence intact; do not edit state or signal remembered PIDs to clear it. If
the executor itself was lost, use the original owned manifest's journaled guest shutdown, preserve its disk/evidence,
verify stopped Mac state, then reconcile. This ends the failed run; it does not repair or resume a product check.

Every passing checkpoint requires execution/task/event receipts bound to its exact attempt. Command-only execution has
explicit fixed-operation and completion events; these are not model turns or native input evidence. GUI passes
additionally require a retained PNG or JPEG whose bytes came from an actual native CUA image result. A hand-written
result, host screenshot, substituted screencapture or clean process exit cannot satisfy that provenance gate. The
receipt binds capture provenance; interpreting whether the pixels prove the product behavior still requires the tester's
observation and independent evaluation. Export includes executor diagnostics and validated evidence references, never
the entire model working directory.

Evidence references may name nested regular files inside the current attempt, such as
`old-signed-inputs/verification.json`. The executor exports only the selected files and creates their private parent
directories; it does not copy neighboring downloads or scratch files. Absolute paths, dot components, symlink
components, authentication-store names and executor-owned record names are refused. Reserved names are compared without
case so a Mac volume cannot alias a model-created directory to an executor record.

The executor saves up to ten recent distinct native image results that fit the remaining reference and byte budgets, as
private `native-capture-*` files. The window-drag check instead retains its ten separate image occurrences, since a
cropped app image can have identical pixels after the window moves. Its fixed read-only window-server measurements pin
the owned window/process, calibrate actual image coordinates and verify moves, zoom/restores and stationary controls.
Those measurements supplement the native images and gestures; they do not replace them. The guest requests screenshots
at the important acceptance states; fixed operations retain command receipts, and the model never encodes or saves
images itself. `final.txt` preserves the original UI observation, `model-result.json` adds machine-owned command
references, and the accepted `result.json` adds native captures. P2 feedback requires two distinct native images, before
and after Send. P2 announcements requires three distinct native images of the bell, open dialog and history. This binds
their bytes to real tool results; review of the images and event sequence must establish their before/after meaning.
Interrupted event rows can be skipped when salvaging failure diagnostics, but successful execution still requires strict
event parsing and a completed turn.

The tester reserves `fail` for actual Farhelm failures. Errors in its own scripts or evidence preparation use
`automation-failure`; missing product prerequisites use `could-not-run`, and OS/CUA/authentication refusals use
`permission-block`. Safe diagnostic observations can have no extra evidence files beyond the executor records; a pass
still requires its declared command/image evidence. Do not relabel an evidence-saving fault as a broken release.
Diagnostic salvage remains strict about references: one invalid file reference can leave only the raw
`model-result.json` and executor records. Inspect that raw claim when judging a failed attempt. A filesystem error
during export can leave termination unacknowledged even after child cleanup; reconcile it rather than treating a partial
archive as complete.

The native event log is bounded to 96 MiB (including base64 images), result evidence to 16 MiB in aggregate, and
collection to 128 MiB. Executor logs live outside the model's working directory. These are accidental-corruption and
provenance checks, not a sandbox against a malicious same-user `--yolo` tester. Review the event stream for prohibited
UI automation or permission grants when judging an actual pass. A tool error stops the attempt; diagnostics and safe
unverified observation files remain available. The host repairs safe problems under "Repair and continue"; recovery is
retained and reported, never silent. `automation.verdict: fail` reports a harness bug separately; release/P0 verdicts
remain `could not run` for it. Only an actual recorded product failure produces the release `fail` verdict. If an
aborted turn nevertheless reported a product failure, or a completed turn reported one whose evidence then failed host
import, `unverified_product_failures` surfaces that observation at run level and in its tier's block (`p0`, `p1` or
`p2`). `promotion_blocked_pending_review: true` requires review of the retained failure claim before any promotion
decision; do not treat it as a harness-only unavailable check. A nonpass native preflight is always a fixture/harness
failure, even if its tester called it `fail` or `could-not-run`. A false review-block flag does not clear an unavailable
or failed automation gate: inspect its retained attempt and resolve that outcome before treating P0 as successful.

Report P0's verdict promptly. The installer pair runs its in-scope P1 checks and, if every runnable P0 check passed and
P2 is in scope, its feedback and announcement checks. Export and clean up that pair, then run the dedicated Mac pair if
P1 is in scope. Report each tier's verdict as it settles. A valid P1 or P2 product failure or unavailable observation
allows ordinary cleanup and later bonus coverage. Its evidence and verdict remain saved. Model, tool or permission
failures stop the current attempt. Follow "Repair and continue". After Mac pair cleanup, stop in stop-after-P1 or
stop-after-P2 scope. Full scope continues to `fresh` and `upgrade-v<previous>` (at most three, newest first) when P0
permits bonus work. Historical manifests without explicit P0 fixtures report `p0.verdict: not planned`, and a tier a run
never declared reports `not planned`. Do not relabel historical observations or add a live send to a run that never
planned it.

The guest tester receives the public helper/recipe and a credential-free public source URL. It checks the previous tag's
source identity, then runs
`verify_release.py --previous v<previous> --version v<candidate> --output <guest-private-directory>` from that source
checkout. For fresh install, use the previous stable named by its plan. This verifies the candidate against the **old**
compiled trust ring and produces `verification.json`. The coordinator requires the correct candidate, previous trust
ring, verified signature/comment/installer checksum and the same checksum-file hash seen at planning. A source build or
verification against candidate/main keys cannot satisfy this check. Install and provision the actual published payloads
inside guests.

The P0 fixtures' checkpoints group these obligations; each passing GUI check also needs command evidence, except the
feedback send:

Fresh-fixture cleanliness uses the supplied `fixture_probe.py`, rather than model-written source searches or service
queries. It checks known install/state locations, absolute XDG overrides, executable names and service listings on Mac
and its pinned peer. On Linux, both systemd managers must answer before an empty listing's exit 1 is treated as no
matches; diagnostics or other failure statuses remain unavailable observations. The host requires the paired concrete
receipt, including all process/service observations. The helper never reads configuration, databases or credentials, and
never repairs residue to manufacture a clean fixture.

| Fixture                | Ordered checkpoints, with their tier                                                                                                                                                                                                                                                                                                          |
| ---------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `p0-fresh`             | P0: `artifact-verification`, `fixture-clean`, `install`, `app-start`                                                                                                                                                                                                                                                                          |
| `p0-updater-upgrade`   | P0: `artifact-verification`, `old-fixture`, `update-discovery`, `update-relaunch` (stable candidates only)                                                                                                                                                                                                                                    |
| `p0-installer-upgrade` | P0: `artifact-verification`, `old-fixture`; P1: `old-sessions`; P0: `update-relaunch`, `local-supervisor-upgrade`, `remote-supervisor-upgrade`, `local-supervisor-agent`, `remote-supervisor-agent`; P1: `local-old-session`, `remote-old-session`, `remote-host-remove`, `remote-host-uninstall`; P2: `p2-feedback-send`, `p2-announcements` |
| `p1-remote-mac`        | P1: `artifact-verification`, `fixture-clean`, `install`, `app-start`, `remote-provision`, `remote-host-uninstall`                                                                                                                                                                                                                             |

`update-discovery` requires the old app's actual in-app check to discover and install the candidate. It cannot pass on a
manual installer path. The installer pair's `old-fixture` provisions the remote host through the old app; the updater
pair's needs none. Each supervisor's upgrade checkpoint requires that supervisor connected at the candidate version with
version evidence (and, for the remote one, its service verified over pinned SSH) before any new agent runs on it. The
two agent checks then require a meaningful response from a genuine agent launched through Farhelm on the corresponding
host. A plain command or CLI version alone cannot satisfy them. The remote agent's new session must keep running: the
Remove and Uninstall checks carry it.

P0 requires no old conversation, template, changed setting or counter continuity workload. P1's old-session checks exist
at the maintainer's request: an upgrading user's running sessions are what an upgrade can break, and P1 shows that a
user upgrading from the previous stable keeps them. In `p0-installer-upgrade`, `old-sessions` runs after the old fixture
and before the upgrade. It starts one genuine Codex session on each host through the old app, submits one prompt,
requires the captured conversation and records the session, conversation and terminal-host identities. Its terminal
sample is refused until that attempt has observed the captured session, and requires exactly one terminal host with one
live child: a previous stable that runs the session anywhere else cannot be tested this way. A failure there happens on
the previous release, so the tester reports it as could not run, and the P0 checks after it still run. Terminal
acceptance covers terminal hosts only: tmux session survival, terminal-kind labels and migration-band checks are
permanently outside this flow. An older app's installer prerequisites do not establish supported terminal behavior in
the candidate. The later checks:

- `local-old-session` and `remote-old-session` follow every P0 checkpoint of the pair, so both supervisors' upgrades,
  including the candidate's automatic upgrade of the remote one, are already established. Each first runs its side's
  one-time survival comparison (`host_survival` or `remote_host_survival`), which takes its own samples before the
  session is touched and compares them with the `old-sessions` receipts. Each check ends with a final sample
  (`old_session_after` or `remote_old_session_after`), refused unless the comparison ran first.
- The old session must survive the upgrade: same session, same launch generation, still captured, and its terminal host
  and that host's child the same processes (pid and start identity) as at `old-sessions`, so a replacement that restores
  the screen and resumes the conversation fails.
- The rediscovered terminal must show its earlier screen and answer a new typed question (whose answer, 48, is not the
  earlier turn's 42 already on screen). Then Restart, which must land on a new terminal host with the old host gone, and
  the resumed agent must name the earlier turn's word.

The coordinator refuses a pass whose fixed samples contradict this: an old session or host that did not survive, or a
Restart without a later generation, without exactly one new host or with the old host still running. Whether the resumed
agent remembers the earlier turn remains the tester's observation. Calls made in the wrong order end in a refusal or an
unavailable check, never a pass. The report folds `old-sessions` into each host's old-session result; with the Remove
and Uninstall checks, those are the installer pair's four named P1 results. This test does not check that the upgrade
ends the old terminals releases before terminal hosts left running; the supervisor's Rust tests cover that startup step.

These checkpoints raise the base budget to at least 1800 seconds for `old-sessions` and each old-session check (within
the 3600-second maximum), since they wait on genuine agent conversations and an agent Restart.

The installer pair's last two P1 checkpoints exercise the Linux host's actual menu items. Remove must forget its
registration and cached sessions while leaving the original supervisor and both live sessions intact; re-adding the same
pinned destination must recover those sessions without provisioning. Uninstall follows after ending all remote sessions
through Farhelm. Its native plan and completion images must establish the removal and kept-data notice. Fixed
before/after SSH snapshots require the service/unit and lib directory gone and no Farhelm process (supervisor or
terminal host) left running, while the data directory and database remain with the same session ids. When the snapshots
contradict a claimed pass from a valid start (lost or unreadable data, something left running, a Remove that changed the
host), the import records a product failure; a failed starting premise stays a harness error. The SSH snapshots observe
the effect of the native menu actions. P2 uses the still-running candidate Mac app and connected local host.

For the fifth P1 result, use the `p1-remote-mac` pair. Its primary Mac runs the candidate app. Its headless peer starts
without the app or any Farhelm state. Provision the peer through **Add host**, verify the candidate version and running
launchd job over pinned SSH, then create and stop one plain-command session there. The starting snapshot must show the
running supervisor and its saved session ID. Use **Uninstall** on the remote Mac's host menu and retain native images of
the plan and kept-data notice. The final SSH snapshot must show the job unloaded, the plist and lib gone, no Farhelm
process remaining, and the data directory and database retaining the same session ID. The
`p1.checks.remote-mac-uninstall` result covers all six checkpoints on this pair. Saved historical manifests keep their
original required checkpoints and named results.

When P2 is in scope, the installer pair ends with `p2-feedback-send` followed by `p2-announcements`. For feedback,
retain a native PNG or JPEG of the filled dialog before Send and another showing it closed after success (or the failure
state), with the agent's actual observation. GUI evidence is sufficient because the helm logs only failed sends. See
RECIPE's P2 section for the fixed message, blank contact and candidate-version/surface premise.

For announcements, the guest checks whether both standing feed files exist on `get.farhelm.io`. A 404 is a P2 failure
with the missing filename, independent of the other checks. An unreachable origin makes this checkpoint unavailable. The
guest launches only the candidate app with `FARHELM_ANNOUNCEMENTS_PATH=announcements-test.json`. That launch reserves
the pending batch for the bell so a fast fetch cannot open it automatically. Capture the rail notice with its action,
the announcement the action opens, and the same entry in Help history after dismissal. A normal relaunch must leave the
override unset. The tester gives the bell up to 60 seconds to show the test notice. If it never arrives, record could
not run with the elapsed wait and the last observed bell state. Either transient delivery trouble or a candidate failure
could leave the bell empty. The announcement checkpoint has a 240-second minimum budget for the delivery window and both
app launches.

The bonus checkpoint names retain RECIPE's broader acceptance obligations:

| Pass    | Checkpoint              | Required observation                                                                           |
| ------- | ----------------------- | ---------------------------------------------------------------------------------------------- |
| Both    | `artifact-verification` | Candidate verified against this pass's previous trust ring.                                    |
| Fresh   | `fixture-clean`         | No Farhelm binaries/state in either guest.                                                     |
| Fresh   | `install`               | Version-path installer output and installed candidate version.                                 |
| Fresh   | `app-start`             | Usable candidate app and connected local host.                                                 |
| Fresh   | `remote-provision`      | Candidate remote supervisor and service evidence.                                              |
| Fresh   | `codex-conversation`    | Meaningful conversation, terminal response and captured conversation.                          |
| Fresh   | `remote-terminal`       | Responsive plain Linux command session.                                                        |
| Fresh   | `window-drag`           | Title-bar band drags and zooms/restores the window; its settings gear still opens settings.    |
| Upgrade | `old-fixture`           | Old app/local/remote versions; automatic updates disabled before first launch.                 |
| Upgrade | `seed-state`            | Real Codex conversation/session identity, template, changed setting and unchanged old version. |
| Upgrade | `continuity-before`     | Original Linux process alive and advancing.                                                    |
| Upgrade | `update-relaunch`       | Actual Restart to update, candidate app/local supervisor and logs.                             |
| Upgrade | `preserved-state`       | Sessions/template/setting retained.                                                            |
| Upgrade | `codex-resume`          | Same captured conversation resumed through Farhelm and responsive.                             |
| Upgrade | `remote-update`         | Candidate remote supervisor and original process continuity.                                   |
| Upgrade | `reopen`                | Normal quit/reopen with versions, hosts, sessions and original process continuity.             |
| Both    | `addendum`              | Filled addendum and applicable extra checks.                                                   |

The tester selects the branch automatically immediately before the first upgrade action. For a separate inspection,
`python3 -B releasing/mac-vm-test/brick_test.py select-update "$test_run" <upgrade-pass>` reobserves `/latest` and saves
`latest-at-selection.json`. Follow the reported `ordinary`, `override` or `manual` branch in RECIPE. Override support
comes from that actual **old** tag's source. `p0-installer-upgrade` always selects the manual installer branch;
`p0-updater-upgrade` uses ordinary or override discovery when supported. An RC or dev build plans no updater pair, which
leaves discovery and strict P0 incomplete. A stable candidate on the manual path fails required discovery and stops that
pair's dependent work; the installer pair still runs once the failed pair is released. Native forwarding, its startup
announcement and removal after relaunch still need actual evidence. Manual installation skips the old updater's probe
and trust checks; separate minisign verification does not erase that coverage gap. If promotion happened meanwhile,
report after-publication coverage. If `/latest` advanced beyond the candidate, stop and replan with the maintainer.

Save an actual native tool-returned PNG or JPEG for every GUI checkpoint. Retain command/service/version/identity
evidence as well for combined GUI and command checks. Bind native computer use to the guest app directly. Capture actual
observations; a screenshot file's existence alone says nothing about whether a product check passed.

Use independent Mac-to-Linux SSH observations for continuity. Retain two advancing pre-update samples for
`continuity-before` before candidate relaunch. Use its last sample as the automatic remote update's baseline, then
sample again at the candidate's connected remote supervisor. Take a separate before/after pair for app quit/reopen.
Receipts must preserve the original `pid`, `start_ticks` and `boot_id`, with a larger `counter`. Do not reboot guests
during that check. See OPERATIONS for the workload and oracle commands.

Record `continuity-before`, then `remote-update`, then `reopen`. Every later pair must identify the original process,
start at or beyond the previous milestone's final counter, and advance further. Saved verdicts recheck this chain;
reusing an old advancing pair or observing a replacement process cannot satisfy another milestone.

The reopen attempt also retains a `*-reopen-transition.json` receipt. It binds the counter samples to the running
candidate before Quit, the native Quit request and the observation that the app exited, and the new app process observed
after managed launch. A saved reopen result without this receipt remains historical evidence and counts as could not run
when the coordinator rechecks it.

## Record evidence and finish

After each named P0, P1 and P2 check, and each planned bonus checkpoint, inspect the recorded result and send the
maintainer an updated Markdown status table. Send it as soon as the result is available, including while `test` is still
running on the same pair. Carry forward earlier results and show the checks still ahead. List all seven named P0 checks,
the five named P1 checks for a new run and both P2 checks, labeled with their tier. Group untouched historical pairs if
there are many, then name the pair and checkpoint as each result comes in.

Use **Passed**, **Failed**, **Intentional skip**, or **Not yet run** for the corresponding results. Give the reason for
each intentional skip, including RC/dev discovery and checks outside a requested stop scope. Use **Could not run** for a
blocked check and name the blocker. Leave its pending dependents **Not yet run**, and mark checks the coordinator
recorded as skipped **Could not run**, with the reason it recorded as the blocker. Report each old session's P1 result
once both `old-sessions` and its own check have settled. The table describes progress. The P0, P1, P2 and full verdicts
still come from the retained evidence and the rules below.

Collect each named guest evidence directory with `control.py collect` before deletion. Inspect the exported tar and its
members. References are relative to `test_run`, either `pass/file.json` or `pass/archive.tar::./member.png`; members are
read without extraction. Never export authentication directories. Give new captures/exports new names instead of
overwriting earlier evidence.

The guest executor imports its own results and native provenance automatically. The `record` interface remains for
historical workflows and explicit failure observations; it cannot manually satisfy a new guest-native pass, and it is
refused while a guest tester holds the pass (running, or awaiting reconciliation), so a hand-made observation cannot
take the place of a result on its way back. For example, a historical run may record already collected evidence with:

```sh
python3 -B releasing/mac-vm-test/brick_test.py record "$test_run" fresh remote-provision \
  --status pass --evidence 'fresh/mac-provision.tar::./remote.png' \
  --evidence 'fresh/linux-service.tar::./service.json' \
  --detail '<actual visible host/version and independently observed service result>'
```

Replace the example observation with what actually happened. A passing GUI check needs a native image; combined checks
also need command evidence. Continuity checks additionally require `--before <reference> --after <reference>`. The
verification checkpoint needs the exported verification receipt. Every passing checkpoint needs nonempty evidence.
Record failures as `--status fail` and unavailable checks as `--status could-not-run`, with diagnostics and the reason.
Accepted observations retain artifact hashes; changed or missing bytes invalidate a later pass verdict. Existing
observations cannot be overwritten. Correct mistakes by retaining the original history and starting a new pass/run after
reconciliation, following operator policy.

A historical host-GUI `reopen` pass also supplies a `*-reopen-transition.json` via `--evidence`. Its `identity` contains
the run's `run_id`, the observed `pass_id`, `check_id` set to `reopen` and `attempt` set to JSON `null`. Its `before`
and `after` each contain the observed `desktop_pids` list and a `sample` matching the corresponding counter receipt. The
`quit` object records `requested: true` and an empty `desktop_pids` list observed before launch. `launched_desktop_pids`
holds the new process list witnessed at launch. Use only retained observations from the same transition.

`python3 -B releasing/mac-vm-test/brick_test.py cleanup "$test_run" <pass>` rechecks successful evidence, cleanly shuts
down both owned guests, verifies stopped state, deletes only unchanged recorded clones and verifies absence. Wait for
the corresponding foreground `serve` session to exit. The controller journals its mutations and outcomes externally;
also journal computer use, decisions and interruptions as operator policy requires.

On the installer pair, ordinary cleanup requires valid P0 evidence and a result for every in-scope P1 and P2 check. A
check that can never run counts: the coordinator records it as skipped, with the reason, when its chain stopped, its
place in the pair's order passed, or (for P2) a P0 check of the run did not pass. On the dedicated Mac pair, ordinary
cleanup requires a result for every in-scope P1 checkpoint. Out-of-scope tiers need no result. If scope narrows before
any tests on a prepared pair, that pair no longer needs a native preflight receipt. A pair with any in-scope check or
existing result, preflight or tester record still needs its preflight evidence. A P1 or P2 product failure can use
ordinary cleanup. Its verdict and evidence are preserved while the pair is cleaned normally. Other failed or incomplete
checks, and any automation failure, keep the retention rule below. Every recorded P1 and P2 result still requires intact
exported evidence, and a failed or unavailable one must have some; changed or missing captures must be recovered before
ordinary cleanup.

Failure or interruption preserves partial manifests and live supervision. Inspect `status`, actual Tart state and the
journal before acting again; preparation is never replayed automatically. Agree failed-clone retention with the
maintainer. Only after that agreement use `cleanup ... --discard-failed`; it preserves the failure verdict and still
requires ownership and clean shutdown. A manifest with an interrupted clone cannot be guessed into deletion authority.
Do not kill a Tart process or force-stop a guest to get around a failed check.

After every planned pass, run `status` and inspect its saved report. **P0 pass** requires all seven critical checks; a
**P1 pass** requires every planned continuity and remote menu result, a **P2 pass** both its planned checks, and a
**full pass** additionally requires P1, P2 and every planned bonus check. Any product failure makes the relevant verdict
fail; missing checks or fixtures make it could not run. In a stop scope, `requested_verdict` combines the tiers it
includes, while the full `verdict` remains incomplete if bonus work did not run. A bonus failure does not erase valid P0
evidence, and a P0 pass does not excuse that bonus failure. Preserve a P0 or bonus failure and repair safe harness/setup
problems under "Repair and continue" before resuming dependent work. For P1 and P2, report the failure promptly, retain
evidence, clean up the pair and continue bonus coverage unless the requested scope ends there. Report exact versions,
checksum hash, update paths, before/after-public-default coverage, omitted agents, diagnostic gaps, cleanup and private
evidence retention. Publish only a redacted summary if separately requested. Never commit profiles, raw evidence, guest
names, private paths, addresses or credentials. This procedure grants no GitHub writes or release promotion.

## Validate changes to this coordinator

Run `python3 -B -m unittest discover -s releasing/mac-vm-test -p 'test_*.py' -v` on an approved test substrate; use the
repository's test-run recorder for durable execution evidence. Tests inject fictional configuration and child-process
boundaries without changing the test process's environment. `python3 -B releasing/mac-vm-test/brick_test.py smoke` uses
the configured real bases for a small headless pair: command readiness, pinned SSH, marker export, clean shutdown and
deletion. Its verdict is explicitly **fixture smoke**, never a release verdict. These targeted checks can validate
boundary refactors while retaining earlier GUI results as historical evidence; they cannot accept new release bytes.
