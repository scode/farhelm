# Brick test a release

Tell a host agent in this checkout: **`brick test release vX.Y.Z`**. It should follow this document, using the private
machine profile discovered below. The host coordinates lifecycle, planning and retained evidence. An independent Codex
tester inside the Mac guest drives genuine native computer use and published installers; it survives Farhelm's own
quit/restart and launches separate authenticated product agents through Farhelm. It does not publish, promote a release
or modify the host's installed Farhelm.

This is a **pre-promotion gate**: run it after the candidate's version-path artifacts are published and before moving
`/latest` to it. The endpoint is expected to name the preceding promoted stable. That difference is not a missing
prerequisite and does not call for promotion; it selects the override or manual update path below. An after-promotion
run remains useful, but cannot gate the candidate's initial exposure.

**P0 checks** are the first product checks after environment readiness and basic coverage planning. Run them in this
order: fresh installation and app startup; installer upgrade from the previous promoted stable; click-to-upgrade
discovery and relaunch from that stable; local supervisor upgrade with one successful genuine agent; remote supervisor
upgrade with one successful genuine agent. Stop on the first failure or unavailable prerequisite. Broader functionality
and historical-version coverage are bonus work after P0 and the separate P1 check, never prerequisites inserted between
these five checks.

**P1** sends one real feedback submission through the candidate app after P0 passes, on the Mac guest of
`p0-updater-upgrade`, before the final export and cleanup of that pair. Its first line is `[brick-test] vX.Y.Z`, naming
the candidate, so the private inbox issue is titled `Feedback: [brick-test] vX.Y.Z`. Report `p1.verdict` and its
`p1-feedback-send` result separately from P0. A P1 failure or unavailable check is reported promptly and does not stop
bonus coverage; P0's verdict remains independent. The full `verdict` includes P1. If P0 fails or cannot run, do not
attempt P1.

The coordinator reports `p0.verdict` and the five named `p0.checks` separately from the full `verdict`. **Did P0 checks
succeed?** means all five ran and passed with retained evidence. **Stop after P0 checks** means record that scope with
`init vX.Y.Z --stop-after-p0`, complete P0, export its evidence, clean up its successful fixtures, and end the session
before P1 or bonus work; no feedback is sent. If requested during a run, persist the scope with
`brick_test.py stop-after-p0 "$test_run"`, finish only the remaining P0 work and cleanup, and retain the request in the
private working log. If bonus work already began, stop that work and preserve its partial evidence; the scope command
fences future fixtures but does not itself cancel GUI actions or shut down guests. A P0 pass does not claim that bonus
checks or every historical upgrade ran.

If P1 was already dispatched when scope narrowed, the send cannot be recalled. Retain its actual result, marked
`dispatched_before_scope_stop`, and report that outcome separately. The stop request prevents another send; it never
erases an in-flight observation. An initial P0-only run never dispatches P1.

**Stop after P1 checks** means use `init vX.Y.Z --stop-after-p1`, finish P0 and P1, retain their evidence, clean up the
last pair even if P1 failed, and stop before bonus fixtures. A mid-run request is persisted with
`brick_test.py stop-after-p1 "$test_run"`; like the P0 scope command, it fences future fixtures rather than cancelling
ongoing GUI work. A run started as stop-after-P0 had no approval for the live feedback send in its preflight, so obtain
that approval before widening it this way. `requested_verdict` then includes P0 and P1 together. **Did P1 checks
succeed?** is answered from `p1.verdict` and the named check, never inferred from P0 or the full verdict.

Widening a run whose final pair was already cleaned cannot execute P1 on that discarded fixture. Start a new
appropriately authorized run rather than restoring a deleted pair or inventing its feedback observation.

Read [RECIPE.md](RECIPE.md) for the product checks and [OPERATIONS.md](OPERATIONS.md) for guest commands, GUI mechanics
and evidence collection. [BRING-UP.md](BRING-UP.md) covers preparing another machine. A previously successful run proves
that run's versions and paths; a new candidate needs its own checks against its actual artifacts.

## Discover the private machine

`brick_test.py` reads the explicit `--profile` argument, then `FARHELM_BRICK_TEST_PROFILE`, then
`~/.config/farhelm/brick-test.json`. A missing named override fails rather than falling back. The profile must be mode
0600, outside the public checkout, and contain exactly this format (these are fictional example bindings):

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
      "model": "gpt-6.1-sol",
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

VM storage (`tart_home/vms`) and evidence must share a filesystem for the single-volume disk budget. The profile loader
and the next-pair preparation both check that invariant, including mounted or symlinked storage. Budget observations
measure the actual VM-storage volume. A split-volume setup needs an explicitly supported accounting scheme; this
coordinator refuses it rather than checking a spacious evidence disk while the VM disk fills.

Format-one profiles support command-only fixture smoke and historical status and cleanup. New release tests require
prepared native fixtures and format two. Both reusable bases must be free of Codex authentication and Farhelm state. The
private control project owns the bases, credential authorization, image provenance, budgets, journal and profile
installation. It may have its own setup adapter. Farhelm does not discover that project by scanning home directories or
depend on its location. Its portable interface is this profile. Installation of the profile is a separate host write to
agree during machine setup. If the profile, approved bases, GUI access or permissions are missing, report **could not
run** and resolve that prerequisite with the maintainer. Do not install host software or alter host networking as a
fallback.

## Plan and start a run

Run commands from the Farhelm checkout with Python 3.11 or newer. Replace the release placeholder with the requested
exact tag. Do not interpret `vX.Y.Z` literally.

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

For full or stop-after-P1 scope, include approval to send the live P1 feedback through the candidate app to
`farhelm.io`: it creates an issue in the maintainer's private feedback inbox, with the fixed brick-test title and no
contact information. Reuse explicit authorization covering that send; otherwise ask alongside the other missing
approvals. A stop-after-P0 scope does not include this send and needs no permission for it.

Record that separate setup approval as `feedback_send_approved: true` in the private authorization record. It authorizes
only RECIPE's fixed brick-test message, sent through the disposable candidate app to `farhelm.io` and its private inbox,
with blank contact. Credential-copy consent does not authorize this send. Full/P1-only initialization and testing refuse
before product mutations without that field; widening scope and the P1 action boundary recheck it. P0-only ignores the
field and never schedules the send. Do not set it without the maintainer's actual authorization.

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
  candidate commit, observes `https://get.farhelm.io/latest`, lists eligible stable upgrades from v0.23.0 onward and
  checks candidate installer metadata availability. Availability and an advertised hash are **not** signature
  verification. Needed payloads and the previous releases' inputs must also exist for their actual guest flows.
- Upgrade baselines run through the stable named by `/latest`, excluding the candidate itself. A newer intervening tag
  that has not been promoted is not another installed stable baseline just because it exists; do not require its
  artifacts or treat its omission as a coverage gap. The candidate's own published artifacts are still required.
- Run `python3 -B releasing/mac-vm-test/brick_test.py init vX.Y.Z`. Save the returned private `run_dir` as `test_run`.
  The output names every pass and required checkpoint. The run gets its own directory under the configured evidence
  root, a profile snapshot, candidate input copies and initial free-space observation. No guests start yet. Add
  `--stop-after-p0` or `--stop-after-p1` when requested; both refuse bonus fixtures, and P0-only also refuses P1
  records.
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
`report.json`. This read-only command is the JSON interface for automation. Lifecycle commands can print guest shutdown
or controller messages before their final report; retain that output as diagnostics rather than parsing it as one JSON
document. A deleted pass retains its evidence and verdict but omits live `guests` handles. Read its `phase` and the
named `p0.checks` from `status` instead of expecting those handles after cleanup.

Each prepared pass exposes `guests`, `ssh` and a shell-quoted `control_prefix` for the existing controller. Use those
live handles, not remembered VM names or DHCP addresses. Only one working pair is allowed at a time. The driver checks
the free-space floor and observed volume growth before another pair; these checks are not a continuous disk quota.
Measure free space during large installations too, following operator policy.

## Perform the product checks

Run RECIPE's P0 procedure first: `p0-fresh`, then `p0-installer-upgrade`, then `p0-updater-upgrade`. The last fixture
holds P0 steps 3–5, in that order. The two upgrade fixtures are independent: a successful installer run cannot prove
discovery by the old app's updater. The coordinator fences later P0 fixtures and bonus work on preceding valid P0
evidence, refuses additional observations after a failed or unavailable check outside P1, and prevents adding extra
feature checks to P0. Export and record each successful fixture, then clean it up before starting the next; retain the
last pair for P1 unless scope is P0-only. Never provision frozen bases.

On the first open after a fresh install, or after upgrading from a release without it, dismiss the welcome card with
**got it** before driving the session UI. This is ordinary startup, not an additional P0 feature fixture.

For each ready fixture, run `python3 -B releasing/mac-vm-test/brick_test.py test "$test_run" <pass>`. Keep its separate
`serve` owner alive. The `test` command injects already-authorized credentials into both disposable roles, checks actual
native screenshot/click/text entry before Farhelm, then executes the ordered checkpoints in fresh guest Codex turns with
`--yolo`. It selects the update branch from a fresh `/latest` observation at the action boundary, imports bounded
private evidence, and stops at the first model, tool or permission failure, or product failure outside P1. There is no
automatic retry. `--timeout` bounds each turn (30–3600 seconds, default 1200). Status remains queryable while a turn
runs; cleanup and another tester refuse an active attempt. If the controller disappears, reconcile the retained claim
and guest process before cleanup; a stale claim is not permission to start a second tester.

If that owner has disappeared, run `brick_test.py reconcile-tester "$test_run" <pass>`. A kernel-held owner lease
refuses interference with a live coordinator. Reconciliation asks the exclusive guest executor to cancel and requires
its stopped acknowledgement, or observes a stopped Mac VM. It retains the failed run and releases only the cleanup
fence. A missing acknowledgement leaves the fence intact; do not edit state or signal remembered PIDs to clear it. If
the executor itself was lost, use the original owned manifest's journaled guest shutdown, preserve its disk/evidence,
verify stopped Mac state, then reconcile. This ends the failed run; it does not repair or resume a product check.

Every passing checkpoint requires execution/task/event receipts bound to its exact attempt. GUI passes additionally
require a retained PNG or JPEG whose bytes came from an actual native CUA image result. A hand-written result, host
screenshot, substituted screencapture or clean process exit cannot satisfy that provenance gate. The receipt binds
capture provenance; interpreting whether the pixels prove the product behavior still requires the tester's observation
and independent evaluation. Export includes executor diagnostics and validated evidence references, never the entire
model working directory.

Evidence references may name nested regular files inside the current attempt, such as
`old-signed-inputs/verification.json`. The executor exports only the selected files and creates their private parent
directories; it does not copy neighboring downloads or scratch files. Absolute paths, dot components, symlink
components, authentication-store names and executor-owned record names are refused. Reserved names are compared without
case so a Mac volume cannot alias a model-created directory to an executor record.

The executor saves up to ten recent distinct native image results that fit the remaining reference and byte budgets, as
private `native-capture-*` files. The guest requests screenshots at the important acceptance states and writes command
receipts; it never encodes or saves images itself. `model-result.json` preserves the original observation, while the
accepted `result.json` adds those captures. The P1 contract requires before-Send and after-Send observations; its
structural gate requires two distinct native images. This binds their bytes to real tool results; review of the images
and event sequence must establish their before/after meaning. Interrupted event rows can be skipped when salvaging
failure diagnostics, but successful execution still requires strict event parsing and a completed turn.

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
unverified observation files remain available, and the harness does not recover silently. `automation.verdict: fail`
reports a harness bug separately; release/P0 verdicts remain `could not run` for it. Only an actual recorded product
failure produces the release `fail` verdict. If an aborted turn nevertheless reported a product failure,
`unverified_product_failures` surfaces that observation at both run and P0 level.
`promotion_blocked_pending_review: true` requires review of the retained failure claim before any promotion decision; do
not treat it as a harness-only unavailable check. A nonpass native preflight is always a fixture/harness failure, even
if its tester called it `fail` or `could-not-run`. A false review-block flag does not clear an unavailable or failed
automation gate: inspect its retained attempt and resolve that outcome before treating P0 as successful.

After P0 succeeds, report its verdict promptly. Unless asked to stop after P0, the guest tester performs RECIPE's P1
feedback send on the candidate still running in `p0-updater-upgrade`. Report its separate verdict, then export and clean
up the pair. A valid P1 product failure or unavailable observation does not require retaining the clones and does not
fence bonus coverage; its evidence and verdict remain saved. Model, tool or permission failures still stop the
automation. P0's per-check exports establish its verdict before P1; the final export adds P1's native captures. Stop
here in P1-only scope. Otherwise continue the full fresh pass (`fresh`) and full upgrade passes (`upgrade-v<previous>`),
newest baseline first, as bonus coverage on separate clones. Historical manifests without explicit P0 fixtures report
`p0.verdict: not planned`; those that never declared P1 report `p1.verdict: not planned`. Do not relabel historical
observations or add a live send to a run that never planned it.

The guest tester receives the public helper/recipe and a credential-free public source URL. It checks the previous tag's
source identity, then runs
`verify_release.py --previous v<previous> --version v<candidate> --output <guest-private-directory>` from that source
checkout. For fresh install, use the previous stable named by its plan. This verifies the candidate against the **old**
compiled trust ring and produces `verification.json`. The coordinator requires the correct candidate, previous trust
ring, verified signature/comment/installer checksum and the same checksum-file hash seen at planning. A source build or
verification against candidate/main keys cannot satisfy this check. Install and provision the actual published payloads
inside guests.

P0's checkpoints group these obligations; each passing GUI check also needs command evidence:

Fresh-fixture cleanliness uses the supplied `fixture_probe.py`, rather than model-written source searches or service
queries. It checks known install/state locations, absolute XDG overrides, executable names and service listings on Mac
and the pinned Linux peer. Both systemd managers must answer before an empty listing's exit 1 is treated as no matches;
diagnostics or other failure statuses remain unavailable observations. The host requires the paired concrete receipt,
including all process/service observations. The helper never reads configuration, databases or credentials, and never
repairs residue to manufacture a clean fixture.

| Fixture                | Ordered checkpoints                                                                                                                |
| ---------------------- | ---------------------------------------------------------------------------------------------------------------------------------- |
| `p0-fresh`             | `artifact-verification`, `fixture-clean`, `install`, `app-start`                                                                   |
| `p0-installer-upgrade` | `artifact-verification`, `old-fixture`, `update-relaunch`                                                                          |
| `p0-updater-upgrade`   | `artifact-verification`, `old-fixture`, `update-discovery`, `update-relaunch`, `local-supervisor-agent`, `remote-supervisor-agent` |

`update-discovery` requires the old app's actual in-app check to discover and install the candidate. It cannot pass on a
manual installer path. The two supervisor/agent checks require connected candidate supervisors, version evidence and a
meaningful response from a genuine agent launched through Farhelm on the corresponding host. A plain command or CLI
version alone cannot satisfy them. P0 requires no template, changed setting, old conversation, resume test or counter
continuity workload.

The final fixture also declares `p1-feedback-send` after its P0 checkpoints. This is GUI-only: retain a native PNG or
JPEG of the filled dialog before Send and another showing it closed after success (or the failure state), with the
agent's actual observation. It needs no command-side success receipt; the helm logs only failed sends. See RECIPE's P1
section for the fixed message, blank contact and candidate-version/surface premise.

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
`p0-updater-upgrade` must use ordinary or override discovery. An unsupported old updater or prerelease that cannot be
discovered before promotion makes P0 could not run; do not replace that check with another manual installation. Native
forwarding, its startup announcement and removal after relaunch still need actual evidence. Manual installation skips
the old updater's probe and trust checks; separate minisign verification does not erase that coverage gap. If promotion
happened meanwhile, report after-publication coverage. If `/latest` advanced beyond the candidate, stop and replan with
the maintainer.

Save an actual native tool-returned PNG or JPEG for every GUI checkpoint. Retain command/service/version/identity
evidence as well for combined GUI and command checks. Bind native computer use to the guest app directly. Capture actual
observations; a screenshot file's existence alone says nothing about whether a product check passed.

Use independent Mac-to-Linux SSH observations for continuity. Retain two advancing pre-update samples for
`continuity-before`, then before/after samples spanning remote Update and app quit/reopen. Receipts must preserve the
original `pid`, `start_ticks` and `boot_id`, with a larger `counter`. Do not reboot guests during that check. See
OPERATIONS for the workload and oracle commands.

Record `continuity-before`, then `remote-update`, then `reopen`. Every later pair must identify the original process,
start at or beyond the previous milestone's final counter, and advance further. Saved verdicts recheck this chain;
reusing an old advancing pair or observing a replacement process cannot satisfy another milestone.

## Record evidence and finish

Collect each named guest evidence directory with `control.py collect` before deletion. Inspect the exported tar and its
members. References are relative to `test_run`, either `pass/file.json` or `pass/archive.tar::./member.png`; members are
read without extraction. Never export authentication directories. Give new captures/exports new names instead of
overwriting earlier evidence.

The guest executor imports its own results and native provenance automatically. The `record` interface remains for
historical workflows and explicit failure observations; it cannot manually satisfy a new guest-native pass. For example,
a historical run may record already collected evidence with:

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

`python3 -B releasing/mac-vm-test/brick_test.py cleanup "$test_run" <pass>` rechecks successful evidence, cleanly shuts
down both owned guests, verifies stopped state, deletes only unchanged recorded clones and verifies absence. Wait for
the corresponding foreground `serve` session to exit. The controller journals its mutations and outcomes externally;
also journal computer use, decisions and interruptions as operator policy requires.

On P0's last pair, cleanup requires valid P0 evidence and an in-scope P1 result recorded as pass, fail or could-not-run.
In P0-only scope it requires no P1 result. P1 failure does not need `--discard-failed`; its verdict and evidence are
preserved while the pair is cleaned normally. Other failed or incomplete checks keep the retention rule below. A
recorded P1 success still requires intact exported evidence; changed or missing captures must be recovered before
ordinary cleanup.

Failure or interruption preserves partial manifests and live supervision. Inspect `status`, actual Tart state and the
journal before acting again; preparation is never replayed automatically. Agree failed-clone retention with the
maintainer. Only after that agreement use `cleanup ... --discard-failed`; it preserves the failure verdict and still
requires ownership and clean shutdown. A manifest with an interrupted clone cannot be guessed into deletion authority.
Do not kill a Tart process or force-stop a guest to get around a failed check.

After every planned pass, run `status` and inspect its saved report. **P0 pass** requires all five critical checks; a
**P1 pass** requires its feedback check, and a **full pass** additionally requires P1 and every planned bonus check. Any
product failure makes the relevant verdict fail; missing checks or fixtures make it could not run. In P0-only scope,
`requested_verdict` follows P0, while the full `verdict` remains incomplete if bonus work did not run. A bonus failure
does not erase valid P0 evidence, and a P0 pass does not excuse that bonus failure. Stop on a failure outside P1,
preserve diagnostics, and resolve it before continuing. For P1, report the failure promptly, retain evidence, clean up
the pair and continue bonus coverage unless the requested scope ends there. Report exact versions, checksum hash, update
paths, before/after-public-default coverage, omitted agents, diagnostic gaps, cleanup and private evidence retention.
Publish only a redacted summary if separately requested. Never commit profiles, raw evidence, guest names, private
paths, addresses or credentials. This procedure grants no GitHub writes or release promotion.

## Validate changes to this coordinator

Run `python3 -B -m unittest discover -s releasing/mac-vm-test -p 'test_*.py' -v` on an approved test substrate; use the
repository's test-run recorder for durable execution evidence. Tests inject fictional configuration and child-process
boundaries without changing the test process's environment. `python3 -B releasing/mac-vm-test/brick_test.py smoke` uses
the configured real bases for a small headless pair: command readiness, pinned SSH, marker export, clean shutdown and
deletion. Its verdict is explicitly **fixture smoke**, never a release verdict. These targeted checks can validate
boundary refactors while retaining earlier GUI results as historical evidence; they cannot accept new release bytes.
