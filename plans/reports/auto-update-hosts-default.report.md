### What this was about

Remote hosts used to wait for Update unless you enabled automatic updates. Farhelm now updates eligible remote hosts
while the app is open. As requested, the default applies to existing installations, including ones that previously
turned the setting off. Turning off **Update remote hosts automatically** under **Settings → Hosts** after the upgrade
saves an opt-out that survives restart.

### Things you should know

- Turning the setting off stops new starts, including an update still being planned. An update that already started can
  finish. Clearing the saved choice through the preferences API restores the default.
- The brick test observes the automatic remote upgrade after the app relaunches. It still checks the connected
  supervisor version, a real agent reply, and the old session's survival and Restart. The bonus continuity check
  compares the original Linux workload against a sample taken before relaunch, even if the update finishes early.
- A devbuild, a test build launched with its own settings, starts with automatic updates off until you enable them. This
  protects shared hosts from an unreleased protocol upgrade that could disconnect the installed app. A saved choice
  survives later devbuilds. An explicit launch using the installed app's settings follows their ordinary policy. Both
  the devbuild launcher and staged binary need this change to supply the private-state default.
- The website pages Manage hosts, Settings, Update or uninstall and Security model describe the new default.

### Open questions and possible follow-ups

- The next release's brick test is the first end-to-end verification of the automatic remote upgrade with real macOS and
  Linux guests. The portable harness tests exercised the evidence and action ordering with stand-ins.
- A power cut can lose a recently saved opt-out. Farhelm can then update remote hosts after restart. The question of
  giving an acknowledged opt-out a stronger durability guarantee remains in `automatic-update-opt-out-can-roll-back.md`.
- Operators can give Farhelm a local directory of build archives to use in place of signed release downloads. The
  automatic update uses the same source as a manual Update. Whether supplying those files should also permit installing
  them without an Update click remains in `staged-payloads-and-automatic-host-updates.md`.

### PRs

- [#4313](https://github.com/scode/farhelm/pull/4313/changes) makes automatic updates the default in the helm and app.
- [#4314](https://github.com/scode/farhelm/pull/4314/changes) observes the automatic upgrade in the brick test.
- [#4315](https://github.com/scode/farhelm/pull/4315/changes) starts devbuilds on separate state with updates off.

### Checks

- `cargo nextest run -p farhelm-helm -p farhelm-gui --lib -E <policy and desktop selection>` covered automatic updates,
  the preference migrations, manual update of an old-protocol host, Settings, host status and attention. It passed 122
  of 123 tests on the initial policy patch over `9ca71cc8`. The section-switch test asserted the old default after a
  refused save. Recorder run `dc0c525c-a92f-40fb-aa07-63f573f5e614`.
- `cargo nextest run -p farhelm-gui -p farhelm-helm --lib -E <focused selection>` passed seven tests after the fix,
  including the Settings regression, migration, opt-out admission, null clearing, old-host advisory and devbuild
  startup. Recorder run `a9d1f227-0548-41c6-921b-8335916049ff` used the devbuild startup patch over `481eb710`. The
  failure came from the expected switch value alone. The focused rerun covers that correction and the other changed
  boundaries, so the wider initial selection was not repeated. The rebase added a test-only SSH polling dependency and
  the later launcher edits received separate checks.
- `python3 -B -m unittest discover -s releasing/mac-vm-test -p 'test_*.py' -v` passed 279 portable tests at `481eb710`.
  Recorder run `cd966dc3-4b37-492e-bd3c-ce15be87b6c6`. The rebase preserved the brick-test patch.
- A `deno test -A <launcher fixture>` check at `96ddb60b` passed four state selections against a stand-in app,
  exercising the actual `--run-only` launcher. Recorder run `0200ea32-1ec2-437d-b30d-6fafc67a7c29`.
  `cargo check --locked -p farhelm -p farhelm-desktop -p farhelm-helm-client --all-targets` passed.
- `bun run build` built 38 website pages with valid internal links. `cargo fmt --all -- --check`, `dprint check` on the
  changed Markdown, changelog format, test-delay lint, launcher syntax and ShellCheck passed.
  `python3 scripts/check-agents-md.py` reported four existing warnings and no errors.
- `python3 scripts/check-gui-goldens.py --update` captured all 298 Linux references. The two host attention captures
  (light and dark) now omit **Always update** while automatic updates are enabled. I inspected both and published them.
  The other 296 images, including both Hosts Settings captures, matched their references. Recorder run
  `5bc4dace-7363-4517-9b88-f0a1052d3ebf`. The run used the policy drawing changes over `9ca71cc8`. The later changes
  kept that drawing code, and the final update to the draft stack changed only those two manifest entries.
- Native macOS launch and release acceptance were deferred to the release fixture. This checkout ran the portable
  coordinator suite and the Linux startup checks.

### Review gate

Each PR received the plan's independent GPT-6.2 Sol high review and a separate cold read of its commit wording. Review
findings were addressed in the corresponding PRs, including stopping production SSH in a fake-host test, waiting for
confirmed remote upgrade completion before sampling, and containing the devbuild flag to its own launch.

### Landing

#### What landed with it

It landed alone in its round. Its base already held the SSH sign-in plan (ssh-askpass-2fa), which added helm database
version 90; this plan's step to clear old opt-outs is version 91 on top of it, and the version constant, the fresh
schema and the step list agree. An independent reviewer and I checked the stack against that plan and found no other
interaction.

#### Fixes made while landing

These went in as a fourth PR, [#4316](https://github.com/scode/farhelm/pull/4316/changes).

- **The installed app would undo a devbuild's host update by itself.** A devbuild reports main's version,
  `0.0.0-unreleased`, which sorts below every release. With automatic updates now on by default, the installed app saw a
  host a devbuild had just updated as out of date and put the release back underneath the devbuild under test,
  restarting that host's supervisor under the devbuild's sessions. Automatic updates now skip a host running a
  development build; manual Update still offers the way back, as the devbuild instructions already said. This is a
  decision I made at landing rather than a plain fix: the other option was only to correct the devbuild instructions and
  accept the revert. Users never run development builds, so it changes nothing for them.
- **The changelog fragment implied the setting could be turned off before anything happened.** On the first launch after
  upgrading, hosts that need an update start updating at once, including for users who had turned the setting off. The
  fragment now says so for the release curator.
- **Wording.** SPEC.md and the Manage hosts page said "this change" and "this version", which go stale; SPEC.md now
  names the development-build exclusion; SPEC_impl.md names the devbuild flag the app reads; the validation inventory no
  longer claims the new test drives the devbuild launcher; and one doc comment is rewrapped.

#### Checks

The plan changes the helm's update policy and its database, the desktop app, the GUI's settings and host status, the
brick-test harness, the devbuild launcher, docs and the website. So the landing ran the whole workspace suite, the
desktop app's tests, the end-to-end smoke and the four clippy runs on the maintainer's own test machine (an arm64 Linux
VM that builds and tests the tree itself), and the goldens, the brick test's portable suite, the devbuild script checks,
the website build and the small checks locally on x86_64.

On the plan as delivered, rebased on main:

- `cargo nextest run --workspace --exclude farhelm-desktop`, on the test machine
  (`f9f6ee90-7711-451f-a53d-7ecf24e5df57`): all 8148 passed, with no runtime skips.
- `cargo nextest run -p farhelm-desktop`, on the test machine (`a0d2c92d-eb4e-4b51-9508-a7f90a028433`): all 176 passed.
- The end-to-end smoke (`python3 -B scripts/gui-e2e-smoke.py`), on the test machine: 49 of 50 steps passed, the
  clipboard paste step skipped as always on Linux (`89e3d7f5-39d0-4e47-9600-6c3f3edecdcc`).
- The four clippy runs, on the test machine: passed. These and the desktop and smoke runs synced a tree that already
  held the one-line development-build exclusion.
- `python3 scripts/check-gui-goldens.py`, locally (`88beeb28-c84d-4f69-bbf5-1a9bc1c1f3df`): passed with the plan's two
  new references.
- `python3 -B -m unittest discover -s releasing/mac-vm-test -p 'test_*.py'`, locally
  (`a7562b64-2eff-480a-bdd0-eaf4fde409c0`): all 279 passed.
- `bash -n scripts/devbuild-mac.sh` and `shellcheck scripts/devbuild-mac.sh`, `cargo fmt --all -- --check`,
  `python -B scripts/check-test-sleeps.py`, `dprint check`, `python3 releasing/check-changelog.py format`,
  `python3 scripts/check-agents-md.py`, `python3 -B scripts/test-gui-e2e-smoke.py`,
  `python3 -B scripts/test-gui-screenshot-batch.py` and the website build (`bun run build`), locally: passed.

On the fixed tree:

- `cargo nextest run --workspace --exclude farhelm-desktop -E 'package(farhelm-helm)'`, on the test machine
  (`aa1b13c0-e9e8-42ea-aea9-da68c82dbcbc`): all 1776 passed, with no runtime skips, including the extended eligibility
  test.
- `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings`, on the test machine:
  passed.
- `cargo fmt --all -- --check`, `dprint check`, `python3 releasing/check-changelog.py format`,
  `python3 scripts/check-agents-md.py` and the website build, locally: passed.

The other runs were reused for the fixed tree: the fixes change one helm rule, a doc comment and documents.

Skipped: anything on a Mac and a real brick test, which stay with the next release, as the report says.

#### Things you should know that the report does not say

- **The brick test can end an attempt on timing alone.** When the tester reads the remote host's version before the
  automatic update has finished, the harness reports "remote binary is not the expected release" as a product failure
  instead of waiting. The fix is a few lines in `releasing/mac-vm-test/guest_operations.py`: for the `remote-update` and
  `remote-supervisor-agent` checks, raise the existing not-ready deferral when the host still reports the previous
  release. I did not make it at landing; the next release's brick test is the first real run, and its
  repair-and-continue rules cover it if it happens.
- **A host shared by two installations on different releases is now updated by the newer one without asking.** After a
  protocol bump the older installation loses that host until it upgrades. This follows from the decision.
- **A saved opt-out is now the only thing that stops automatic updates**, so the known power-loss risk for a recently
  saved opt-out (the report's follow-up) matters more than before.
- **No reference screenshot shows Always update any more.** The host rail picture used to show it as a side effect of
  the unset preference; it still appears after an explicit opt-out.
- **A devbuild started on the installed app's own state gets no protection**, and the launcher prints nothing about it.
  The report discloses this.
