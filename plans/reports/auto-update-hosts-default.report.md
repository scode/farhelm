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
