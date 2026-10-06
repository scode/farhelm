# Brick test a release

Tell a host agent in this checkout: **`brick test release vX.Y.Z`**. It should follow this document, using the private
machine profile discovered below. This is an agent-driven test with genuine computer use, authenticated Codex sessions
and published release bytes. The coordinator supplies lifecycle, planning and evidence gates; the agent performs and
attests the product checks. It does not publish, promote a release or modify the host's installed Farhelm.

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

- Confirm release tags are available in the checkout; fetch missing tags from its normal Git remote if authorized.
  `python3 -B releasing/mac-vm-test/brick_test.py plan vX.Y.Z` performs read-only release discovery. It resolves the
  candidate commit, observes `https://get.farhelm.io/latest`, lists eligible stable upgrades from v0.23.0 onward and
  checks candidate installer metadata availability. Availability and an advertised hash are **not** signature
  verification. Needed payloads and the previous releases' inputs must also exist for their actual guest flows.
- Run `python3 -B releasing/mac-vm-test/brick_test.py init vX.Y.Z`. Save the returned private `run_dir` as `test_run`.
  The output names every pass and required checkpoint. The run gets its own directory under the configured evidence
  root, a profile snapshot, candidate input copies and initial free-space observation. No guests start yet.
- Read the candidate's release notes, changelog fragments and protocol/schema changes. Fill RECIPE's addendum inside
  `test_run`. Record all planned pairs, Codex selection, omitted agents and concrete extra checks. Add each extra
  obligation with `brick_test.py add-check "$test_run" <pass> extra-<name>`, adding `--gui` for a GUI check. Baseline
  checkpoints cannot be replaced. If historical tags or bytes are missing, report the gap; do not silently narrow the
  upgrade range and claim a standard pass.
- In a supervised foreground tool session, run `python3 -B releasing/mac-vm-test/brick_test.py serve "$test_run" fresh`.
  Keep this session alive. The driver verifies both frozen bases' identities and stopped states before cloning, starts
  the owned pair, establishes command readiness and pinned guest-to-guest SSH, and stages
  `~/farhelm-brick-test/continuity.py` in Linux. Wait for its ready report. Linux is headless; macOS has a Tart window
  for computer use. The `--headless` option is for command-only work, not a substitute for desktop evidence.

Use `python3 -B releasing/mac-vm-test/brick_test.py status "$test_run"` to inspect progress and save private
`report.json`. Each prepared pass exposes `guests`, `ssh` and a shell-quoted `control_prefix` for the existing
controller. Use those live handles, not remembered VM names or DHCP addresses. Only one working pair is allowed at a
time. The driver checks the free-space floor and observed volume growth before another pair; these checks are not a
continuous disk quota. Measure free space during large installations too, following operator policy.

## Perform the product checks

Run RECIPE's fresh pass, then its upgrade pass for **each** planned previous version. After exporting and recording one
successful pass, clean it up before starting the next with `serve "$test_run" upgrade-v<previous>`. Each upgrade gets
new clones. Never start or provision frozen bases.

For each pass, copy the needed verification helper and tag source into the Mac guest through the documented command
channel. Run `verify_release.py --previous v<previous> --version v<candidate> --output <guest-private-directory>` from
that guest source checkout. For fresh install, use the previous stable named by its plan. This verifies the candidate
against the **old** compiled trust ring and produces `verification.json`. Export it and record `artifact-verification`.
The coordinator requires the correct candidate, previous trust ring, verified signature/comment/installer checksum and
the same checksum-file hash seen at planning. A source build or verification against candidate/main keys cannot satisfy
this check. Install and provision the actual published payloads inside guests.

The checkpoint names group RECIPE obligations; they do not reduce them:

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
`python3 -B releasing/mac-vm-test/brick_test.py select-update "$test_run" upgrade-v<previous>`. It reobserves `/latest`
and saves `latest-at-selection.json`. Follow the reported `ordinary`, `override` or `manual` branch in RECIPE. Override
support comes from that actual **old** tag's source. Native forwarding, its startup announcement and removal after
relaunch still need actual evidence. Manual installation skips the old updater's probe and trust checks; separate
minisign verification does not erase that coverage gap. If promotion happened meanwhile, report after-publication
coverage. If `/latest` advanced beyond the candidate, stop and replan with the maintainer.

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

After every planned pass, run `status` and inspect its saved report. **Pass** requires all baseline and extra checks
across all planned pairs. Any product failure makes the verdict fail; missing checks or fixtures make it could not run.
Report exact versions, checksum hash, update paths, before/after-public-default coverage, omitted agents, diagnostic
gaps, cleanup and private evidence retention. Publish only a redacted summary if separately requested. Never commit
profiles, raw evidence, guest names, private paths, addresses or credentials. This procedure grants no GitHub writes or
release promotion.

## Validate changes to this coordinator

Run `python3 -B -m unittest discover -s releasing/mac-vm-test -p 'test_*.py' -v` on an approved test substrate; use the
repository's test-run recorder for durable execution evidence. Tests inject fictional configuration and child-process
boundaries without changing the test process's environment. `python3 -B releasing/mac-vm-test/brick_test.py smoke` uses
the configured real bases for a small headless pair: command readiness, pinned SSH, marker export, clean shutdown and
deletion. Its verdict is explicitly **fixture smoke**, never a release verdict. These targeted checks can validate
boundary refactors while retaining earlier GUI results as historical evidence; they cannot accept new release bytes.
