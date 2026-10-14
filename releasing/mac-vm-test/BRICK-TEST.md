# Brick test a release

Tell a host agent in this checkout: **`brick test release vX.Y.Z`**. It should follow this document, using the private
machine profile discovered below. This is an agent-driven test with genuine computer use, authenticated Codex sessions
and published release bytes. The coordinator supplies lifecycle, planning and evidence gates; the agent performs and
attests the product checks. It does not publish, promote a release or modify the host's installed Farhelm.

This is a **pre-promotion gate**: run it after the candidate's version-path artifacts are published and before moving
`/latest` to it. The endpoint is expected to name the preceding promoted stable. That difference is not a missing
prerequisite and does not call for promotion; it selects the override or manual update path below. An after-promotion
run remains useful, but cannot gate the candidate's initial exposure.

**P0 checks** are the first product checks after environment readiness and basic coverage planning. Run them in this
order: fresh installation and app startup; installer upgrade from the previous promoted stable; click-to-upgrade
discovery and relaunch from that stable; local supervisor upgrade with one successful genuine agent; remote supervisor
upgrade with one successful genuine agent. Stop on the first failure or unavailable prerequisite. Broader functionality
and historical-version coverage are bonus work after P0, never prerequisites inserted between these five checks.

The coordinator reports `p0.verdict` and the five named `p0.checks` separately from the full `verdict`. **Did P0 checks
succeed?** means all five ran and passed with retained evidence. **Stop after P0 checks** means record that scope with
`init vX.Y.Z --stop-after-p0`, complete P0, export its evidence, clean up its successful fixtures, and end the session
before bonus work. If requested during a run, persist the scope with `brick_test.py stop-after-p0 "$test_run"`, finish
only the remaining P0 work and cleanup, and retain the request in the private working log. If bonus work already began,
stop that work and preserve its partial evidence; the scope command fences future fixtures but does not itself cancel
GUI actions or shut down guests. A P0 pass does not claim that bonus checks or every historical upgrade ran.

Read [RECIPE.md](RECIPE.md) for the product checks and [OPERATIONS.md](OPERATIONS.md) for guest commands, GUI mechanics
and evidence collection. [BRING-UP.md](BRING-UP.md) covers preparing another machine. A previously successful run proves
that run's versions and paths; a new candidate needs its own checks against its actual artifacts.

## Discover the private machine

`brick_test.py` reads the explicit `--profile` argument, then `FARHELM_BRICK_TEST_PROFILE`, then
`~/.config/farhelm/brick-test.json`. A missing named override fails rather than falling back. The profile must be mode
0600, outside the public checkout, and contain exactly this format (these are fictional example bindings):

```json
{
  "format": 1,
  "mac_base": { "name": "mac-test-base", "mac_address": "02:00:00:00:00:01" },
  "linux_base": { "name": "linux-test-base", "mac_address": "02:00:00:00:00:02" },
  "linux_user": "guest",
  "tart_home": "~/.tart",
  "journal": "operator/LOG.md",
  "evidence_root": "~/.tart/farhelm-evidence",
  "operator_policy": "operator/AGENTS.md",
  "resource_limits": { "minimum_free_gib": 100, "maximum_run_growth_gib": 60 },
  "route_via_gateway": false,
  "agents": ["codex"]
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

The private control project owns the bases, authentication, image provenance, budgets, journal and profile installation.
It may have its own setup adapter. Farhelm does not discover that project by scanning home directories or depend on its
location. Its portable interface is this profile. Installation of the profile is a separate host write to agree during
machine setup. If the profile, approved bases, GUI access or permissions are missing, report **could not run** and
resolve that prerequisite with the maintainer. Do not install host software or alter host networking as a fallback.

## Plan and start a run

Run commands from the Farhelm checkout with Python 3.11 or newer. Replace the release placeholder with the requested
exact tag. Do not interpret `vX.Y.Z` literally.

### Ask for required permissions up front

During environment readiness and basic coverage planning, inventory the permissions needed for the **requested scope**
before starting product checks. Read the profile, operator policy and previous provisioning/authentication records;
inspect available prerequisites without printing credentials or starting frozen bases. Include the genuine agent CLI and
authentication on **both** Mac and Linux, even though their P0 checks run later. A stale copied login may need a
refresh; a successful `login status` is not proof that it can serve a conversation.

Make any necessary approval request concrete: identify the candidate/run, credential source and each working-clone
destination, CLI installation source, guest GUI permissions, and the bounded private evidence to retain and where it
will go. Review the planned operations first, reuse explicit authorization that already covers them, and ask for the
remaining approvals together. Credential approval must name Mac and Linux separately; authorization for one does not
cover the other. Scope copies to this run's disposable clones with mode 0600, keep credentials out of logs and exports,
and leave frozen bases unchanged. Record the scope and response privately without credential contents.

Resolve known permission prerequisites before installing or launching Farhelm. Readiness probes in owned clones may
reveal a missing requirement that the provisioning records could not establish; settle it before product checks rather
than deferring it until the corresponding supervisor/agent step. This is permission planning, not an extra agent
acceptance test before fresh installation. Keep actual setup beside the P0 step it supports and retain P0's order.

Later unexpected permission requests still need their own authorization. This preflight neither grants blanket consent
nor bypasses automatic approval review; it moves foreseeable requests to startup so testing can proceed without those
interruptions. In P0-only scope, do not request permissions solely for bonus work.

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

This check belongs at startup because separate Tart processes share an application identity. Computer use can bind to an
earlier process even when the new guest window is in the foreground. After the new Mac clone starts, verify computer use
reports its exact window title before installing Farhelm or seeding state. Resolve a wrong binding at that point instead
of discovering it after fixture preparation.

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
  `--stop-after-p0` when requested; the coordinator refuses bonus fixtures in that scope.
- Read the candidate's release notes, changelog fragments and protocol/schema changes. Fill RECIPE's addendum inside
  `test_run`. Record all planned pairs, Codex selection, omitted agents and concrete extra checks. Add each extra
  obligation on a bonus pass with `brick_test.py add-check "$test_run" <pass> extra-<name>`, adding `--gui` for a GUI
  check. Keep P0's prerequisites minimal; defer historical-source investigation and feature-specific fixtures until P0
  passes. Missing older inputs are a bonus coverage gap, not a reason to delay the previous-stable P0 checks.
- In a supervised foreground tool session, run
  `python3 -B releasing/mac-vm-test/brick_test.py serve "$test_run" p0-fresh`. Keep this session alive. The driver
  verifies both frozen bases' identities and stopped states before cloning, starts the owned pair, establishes command
  readiness and pinned guest-to-guest SSH, and stages `~/farhelm-brick-test/continuity.py` in Linux. Wait for its ready
  report. Linux is headless; macOS has a Tart window for computer use. The `--headless` option is for command-only work,
  not a substitute for desktop evidence.

Use `python3 -B releasing/mac-vm-test/brick_test.py status "$test_run"` to inspect progress and save private
`report.json`. Each prepared pass exposes `guests`, `ssh` and a shell-quoted `control_prefix` for the existing
controller. Use those live handles, not remembered VM names or DHCP addresses. Only one working pair is allowed at a
time. The driver checks the free-space floor and observed volume growth before another pair; these checks are not a
continuous disk quota. Measure free space during large installations too, following operator policy.

## Perform the product checks

Run RECIPE's P0 procedure first: `p0-fresh`, then `p0-installer-upgrade`, then `p0-updater-upgrade`. The last fixture
holds P0 steps 3–5, in that order. The two upgrade fixtures are independent: a successful installer run cannot prove
discovery by the old app's updater. The coordinator fences later P0 fixtures and bonus work on preceding valid P0
evidence, refuses additional observations after a failed or unavailable check, and prevents adding extra feature checks
to P0. Export and record each successful fixture, then clean it up before starting the next. Never provision frozen
bases.

After P0 succeeds, report its verdict promptly. Unless asked to stop after P0, continue RECIPE's full fresh pass
(`fresh`) and full upgrade passes (`upgrade-v<previous>`), newest baseline first, as bonus coverage. These also get
separate clones. Historical manifests without explicit P0 fixtures report `p0.verdict: not planned`; do not relabel
their observations or erase history to fit the new procedure.

For each pass, copy the needed verification helper and tag source into the Mac guest through the documented command
channel. Run `verify_release.py --previous v<previous> --version v<candidate> --output <guest-private-directory>` from
that guest source checkout. For fresh install, use the previous stable named by its plan. This verifies the candidate
against the **old** compiled trust ring and produces `verification.json`. Export it and record `artifact-verification`.
The coordinator requires the correct candidate, previous trust ring, verified signature/comment/installer checksum and
the same checksum-file hash seen at planning. A source build or verification against candidate/main keys cannot satisfy
this check. Install and provision the actual published payloads inside guests.

P0's checkpoints group these obligations; each passing GUI check also needs command evidence:

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

Immediately before selecting an upgrade, run
`python3 -B releasing/mac-vm-test/brick_test.py select-update "$test_run" <upgrade-pass>`. It reobserves `/latest` and
saves `latest-at-selection.json`. Follow the reported `ordinary`, `override` or `manual` branch in RECIPE. Override
support comes from that actual **old** tag's source. `p0-installer-upgrade` always selects the manual installer branch;
`p0-updater-upgrade` must use ordinary or override discovery. An unsupported old updater or prerelease that cannot be
discovered before promotion makes P0 could not run; do not replace that check with another manual installation. Native
forwarding, its startup announcement and removal after relaunch still need actual evidence. Manual installation skips
the old updater's probe and trust checks; separate minisign verification does not erase that coverage gap. If promotion
happened meanwhile, report after-publication coverage. If `/latest` advanced beyond the candidate, stop and replan with
the maintainer.

Save a PNG for every GUI checkpoint using guest `screencapture` when authorized. Retain command/service/version/identity
evidence as well for combined GUI and command checks. Bind computer use to the exact Mac clone's Tart window. Capture
actual observations; a screenshot file's existence alone says nothing about whether a product check passed.

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

For example, after collecting the actual fresh GUI and command evidence:

```sh
python3 -B releasing/mac-vm-test/brick_test.py record "$test_run" fresh remote-provision \
  --status pass --evidence 'fresh/mac-provision.tar::./remote.png' \
  --evidence 'fresh/linux-service.tar::./service.json' \
  --detail '<actual visible host/version and independently observed service result>'
```

Replace the example observation with what actually happened. A passing GUI check needs a PNG; combined checks also need
command evidence. Continuity checks additionally require `--before <reference> --after <reference>`. The verification
checkpoint needs the exported verification receipt. Every passing checkpoint needs nonempty evidence. Record failures as
`--status fail` and unavailable checks as `--status could-not-run`, with diagnostics and the reason. Accepted
observations retain artifact hashes; changed or missing bytes invalidate a later pass verdict. Existing observations
cannot be overwritten. Correct mistakes by retaining the original history and starting a new pass/run after
reconciliation, following operator policy.

`python3 -B releasing/mac-vm-test/brick_test.py cleanup "$test_run" <pass>` rechecks successful evidence, cleanly shuts
down both owned guests, verifies stopped state, deletes only unchanged recorded clones and verifies absence. Wait for
the corresponding foreground `serve` session to exit. The controller journals its mutations and outcomes externally;
also journal computer use, decisions and interruptions as operator policy requires.

Failure or interruption preserves partial manifests and live supervision. Inspect `status`, actual Tart state and the
journal before acting again; preparation is never replayed automatically. Agree failed-clone retention with the
maintainer. Only after that agreement use `cleanup ... --discard-failed`; it preserves the failure verdict and still
requires ownership and clean shutdown. A manifest with an interrupted clone cannot be guessed into deletion authority.
Do not kill a Tart process or force-stop a guest to get around a failed check.

After every planned pass, run `status` and inspect its saved report. **P0 pass** requires all five critical checks; a
**full pass** additionally requires every planned bonus check. Any product failure makes the relevant verdict fail;
missing checks or fixtures make it could not run. In P0-only scope, `requested_verdict` follows P0, while the full
`verdict` remains incomplete if bonus work did not run. A bonus failure does not erase valid P0 evidence, and a P0 pass
does not excuse that bonus failure. Stop on a failure, preserve diagnostics, and resolve it before continuing. Report
exact versions, checksum hash, update paths, before/after-public-default coverage, omitted agents, diagnostic gaps,
cleanup and private evidence retention. Publish only a redacted summary if separately requested. Never commit profiles,
raw evidence, guest names, private paths, addresses or credentials. This procedure grants no GitHub writes or release
promotion.

## Validate changes to this coordinator

Run `python3 -B -m unittest discover -s releasing/mac-vm-test -p 'test_*.py' -v` on an approved test substrate; use the
repository's test-run recorder for durable execution evidence. Tests inject fictional configuration and child-process
boundaries without changing the test process's environment. `python3 -B releasing/mac-vm-test/brick_test.py smoke` uses
the configured real bases for a small headless pair: command readiness, pinned SSH, marker export, clean shutdown and
deletion. Its verdict is explicitly **fixture smoke**, never a release verdict. These targeted checks can validate
boundary refactors while retaining earlier GUI results as historical evidence; they cannot accept new release bytes.
