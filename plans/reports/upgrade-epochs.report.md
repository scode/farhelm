# Supported update sources have a minimum version

## What this was about

Farhelm promised normal updates from every stable release since v0.23.0, indefinitely. A future release could therefore
not rely on update logic that an earlier release had added. The maintainer chose a minimum supported source version,
called the upgrade floor, instead of epoch metadata or another host state. It starts at v0.23.0. The periods between
raises are upgrade epochs; a raise is made by the change that needs earlier update logic and accepted when a release is
requested.

The installer now refuses an older installation before starting new update or rebuild work, through both Terminal and
app updates. The app recognizes the dedicated refusal and keeps uninstall-first recovery advice. The helm refuses an
older remote supervisor both when planning Update and when confirming it, including hosts on another protocol version.
The specification and release checks follow that boundary, and the completed TODO entry is removed.

## Things you should know

- The floor governs update mechanics, not whether every feature can read older data. Local uninstall keeps data; remote
  recovery can keep it too. Reinstalling makes no additional data-compatibility promise.
- Development builds and readable unparseable versions are exempt. Published dev releases and RCs use ordinary SemVer
  ordering, so the floor release's own RC is below it. An existing app with a missing or unreadable installed-version
  record refuses conservatively. Ownership and stale-expectation checks still run first; recovery of an interrupted
  install still precedes inspection.
- Older apps show their existing generic failure and retain the installer's explanation in their log. The new app's
  lasting warning distinguishes an unsupported source from verification failure, which keeps its existing reinstall
  advice. A manual removal fallback covers releases without uninstall and damaged records that prevent uninstall from
  starting. Remote advice names the answering installation and protects shared binary directories.
- A release request surfaces a floor raise before work starts, without an enforced age or release count. Changelog dates
  record when notes were written, so actual promotion date, age or count is reported as unknown when history cannot
  establish it. Choosing an earlier stable source pauses the existing main-based release flow; a compatible release path
  must be settled then. This plan adds no historical-source release mechanism.
- Before planning a real brick test, the operator must compare the candidate tag's floor with the coordinator's literal
  and stop on disagreement. The automated Rust/Python agreement test checks one checkout; it cannot establish that a
  separately named candidate has the same floor.

## Open questions and possible follow-ups

No decision remains open for this plan. Legacy bundle rebuild code is deliberately retained, with its race coverage;
removing it is a separate cleanup. Native Mac update and remote-host recovery were not exercised here.

## The PRs

- [#3236](https://github.com/scode/farhelm/pull/3236/changes) — defines the supported update boundary and recovery
  contract.
- [#3239](https://github.com/scode/farhelm/pull/3239/changes) — enforces it in the installer and desktop updater.
- [#3241](https://github.com/scode/farhelm/pull/3241/changes) — enforces it for remote Update with manual recovery
  advice.
- [#3242](https://github.com/scode/farhelm/pull/3242/changes) — aligns release decisions and brick-test coverage, and
  removes the completed TODO entry.

All four remain drafts in one linear stack for the landing monitor.

## Checks run, reused and skipped

Runtime checks ran through the recorder on Linux, with complete source and retained-output evidence. Nextest was pinned
to 0.9.148, four slots and zero retries. None of these selections uses tmux; no runtime skip was reported.

- `cargo nextest run -p farhelm-proto -p farhelm-supervisor -p farhelm-ui --features farhelm-ui/desktop --lib -E 'test(/^version::/) | test(/^installation::install::tests::/) | test(/^desktop::updater::tests::/)'`:
  98 passed, run `3bcc77ea-95e6-441c-b6c6-1137ec4631ca`, based on `256a83f9` plus the captured installer/app working
  tree. Final installer fixtures use fixed floors; their 24 cases passed again with
  `cargo nextest run -p farhelm-supervisor --lib -E 'test(/^installation::install::tests::/)'` in
  `7d29cfbe-bda2-4875-b67f-42048614547f`. These counts overlap. An earlier 26-case pre-correction pass
  (`c192f676-2a2b-488b-be43-4d4fb5ac1df4`) is retained but superseded by these checks.
- `cargo build -p farhelm` passed (`a08d85e7-33ca-46c0-982d-b38767606a9f`), then
  `bash scripts/test-install-sh.sh --farhelm target/debug/farhelm`: 601 checks, zero failures
  (`9b29bf8c-36da-4a9e-97a3-0d25d1c4f4fb`). This uses private fixture homes and a Darwin shim on Linux, not a native Mac
  installation. It verifies status propagation and preservation of legacy app and command files.
- `cargo nextest run -p farhelm-helm --lib -E 'test(/^provisioning::tests::upgrade_floor_/) | test(/^provisioning::tests::update_refuses_/) | test(/^provisioning::uninstall_refusal::/)'`:
  9 passed (`f40953c3-3891-4ec0-8ac8-4858a0d5e9c5`), based on `5704d8ee` plus the captured remote-gate working tree.
  Both planning and confirmation paths are covered, with no host mutation or registry change on refusal.
- `python3 -B -m unittest discover -s releasing/mac-vm-test -p 'test_*.py' -v`: 260 passed
  (`ea43bb97-b04b-49cb-959b-3028656d4669`), based on `5704d8ee` plus the captured release-check working tree. After
  fixing one additional historical fixture's floor,
  `python3 -B -m unittest discover -s releasing/mac-vm-test -p 'test_guest_tester.py' -v`: 34 passed
  (`8964946e-12b5-4e71-ae51-1ef3272bc0fe`). These use fictional substrates and establish harness contracts, not a native
  GUI or release pass. Later corrections were prose and a docstring only.
- Clippy passed:
  `cargo clippy -p farhelm-proto -p farhelm-supervisor -p farhelm-ui --features farhelm-ui/desktop --all-targets -- -D warnings`,
  `cargo clippy -p farhelm-supervisor --lib --tests -- -D warnings`, and
  `cargo clippy -p farhelm-helm --all-targets -- -D warnings`. The careful rebase brought the Rust 1.99.1 pin, launcher
  label correction, plan tooling and bookkeeping. The pin selects the same compiler already used. The tested runtime
  inputs are unchanged; per-PR comparison found only the mechanical TODO conflict, preserving both completed-entry
  removals. The runtime and Clippy results above are reused on that basis.
- After rebase, `cargo fmt --all -- --check`, `git diff --check`, `python3 releasing/check-changelog.py format` and
  `python3 scripts/check-agents-md.py` passed. The instruction check retains one unrelated warning about a missing
  test-authoring heading. The isolated `python -B scripts/check-test-sleeps.py` inspected 430 calls with no missing
  rationale. Changed Markdown passed
  `dprint check SPEC.md SPEC_impl.md docs/install_uninstall.md .agents/run-all-tests.md .agents/validation.md TODO.md releasing/AGENTS.md releasing/.agents/stable.md releasing/mac-vm-test/BRICK-TEST.md releasing/mac-vm-test/RECIPE.md`.
  Earlier `bash -n scripts/test-install-sh.sh` and `shellcheck scripts/test-install-sh.sh` passed.
- Skipped the full Rust and browser batteries, native desktop smoke, actual Mac brick tests, hosted CI and release
  actions: the focused checks cover the changed mechanisms, and the rebase introduced no uncovered runtime interaction.
  No signing, promotion, live installation or service-manager action was performed.

## Review gate's outcome

Each PR passed its required single fresh gpt-6.1-sol/high adversarial review. Findings corrected installer guard order,
legacy shell expectations, neutral app wording, candidate/coordinator floor agreement and promotion-date provenance. No
finding remains. Fresh scope reviews retained the small design and the explicit earlier-base boundary. Commit and PR
wording passed separate cold reads, and every touched file received a separate documentation pass.

### Landing

Landed second of three in its round, between stop-start-message (Stop during an agent's startup now says so) and
session-switch-jiggle (terminal text no longer shifting when you switch sessions); since its base, main had gained only
the fixed-height Settings dialog and review-queue bookkeeping. A fresh-context review read all of them against each
other and found no interaction: no protocol or database schema change in any of the three, and the plans edit different
parts of SPEC.md, SPEC_impl.md and the test inventories. It also checked the floor itself. The installer tells an old
installation by its app layout: the side-by-side layout shipped in v0.22.0, so an installation without it really is
older than the 0.23.0 floor, and nothing at or above the floor is refused for its layout. Development builds and
versions that cannot be read are never treated as below the floor. A floor release's own release candidates count as
below it, as the plan decided. The installer's new refusal has its own exit status, distinct from the ones that mean
"retry". Nothing about tags, signing, publication or `/latest` changed.

#### Fixes made at landing

- The installer's refusal for an installation that is too old to update told the user to delete the app and its
  command-line links at their default locations, and to reinstall with the plain `curl ... | sh` line. Someone who
  installed with `FARHELM_INSTALL_DIR` would have been told to delete the wrong files, possibly another installation's.
  The advice now names this installation's actual app and link paths, and says to install again the way they installed
  before, with the default command as an example (#3239). The installer tests passed after the change, but no test
  asserts the paths in that advice, so the custom-folder case is checked by reading the code only.
- The changelog entry was marked `added`. It takes away updating in place for installations older than v0.23.0, so it is
  now `breaking` (#3241).

#### Things you should know

- The docs website's pages on updating Farhelm and updating hosts do not mention either refusal yet.
- The recovery advice for a remote host says to remove Farhelm's program directory. If someone's data directory sits
  inside that directory, following the advice would delete the data. Provisioning never sets it up that way, so this
  needs a hand-made layout.
- In `releasing/.agents/stable.md`, this plan added that a floor raise is compared against the proposed base when the
  release is requested, and that choosing an earlier base instead pauses the procedure. The same paragraph still says
  the base is never a question and is always the changelog PR's merge commit, which is settled later. Both rules hold at
  once only if the floor comparison is repeated against the final base; as written, an agent cutting the next stable
  release may read them as contradicting each other. Worth a look before the next stable release.
- Otherwise the executor's report still holds; its description of the refusal advice and of the changelog entry predates
  the two fixes above.
- A future floor raise past 0.27.0 will break the installer test fixtures, which use versions 0.27.0 to 0.31.0, so that
  PR has to update them.

#### Checks

The three plans were checked together, stacked in queue order on main (`db0750fb` at the top), on this development
machine and, at the maintainer's request, on Tensorlake sandboxes.

Run on this machine:

- `cargo fmt --all -- --check`, `dprint check`, `python3 releasing/check-changelog.py format`,
  `python3 scripts/check-agents-md.py`, the test-sleep checker, and
  `sh -n scripts/install.sh && shellcheck scripts/install.sh scripts/test-install-sh.sh` passed.
- `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`,
  `cargo check -p farhelm-ui --features desktop` and `cargo check -p farhelm-desktop` passed.
- Recorded `cargo nextest run -p farhelm-ui --features desktop` (the desktop app's own code, including its updater):
  1111 passed, run `7ea46bc3`.
- `bash scripts/test-install-sh.sh --farhelm target/debug/farhelm` (the macOS installer driven against fake releases):
  601 checks passed. This and the next two items are their own harnesses, run directly rather than through the test
  recorder, so they have no run id; their output was read in full.
- The release brick-test harness's unit tests
  (`python3 -B -m unittest discover -s releasing/mac-vm-test -p 'test_*.py'`) passed.
- The UI's JavaScript unit tests (`node --test` in `crates/farhelm-ui/js-tests`) and the grid terminal component's own
  browser tests (`npx playwright test --config grid-client/playwright.config.ts`, Chromium and WebKit, no stack): 200
  passed.

Run on Tensorlake sandboxes:

- Twelve terminal-related browser spec files on Chromium and WebKit, 24 file-and-engine pairs over ten sandboxes, using
  the release web build: `terminal`, `terminal-switch-geometry`, `grid-terminal`, `terminal-font`, `sidebar-resize`,
  `terminal-reconnect`, `terminal-migration`, `terminal-tabs`, `terminal-restart`, `terminal-attachments`,
  `terminal-multihost` and `interface-fonts`. 502 passed, none failed, skipped or retried; runs `c1719814`, `bdda5312`,
  `abf8c3cb`, `480bc664`, `83a819ff`, `91906e53`, `4306834f`, `44a7f61a`, `04e28cba` and `da840416`, in the recorder's
  plain command mode (its Playwright mode cannot pick one engine per file) under the sandbox's slightly older Node 24.
  Every sandbox reported its per-user systemd manager and its SSH to itself working.
- Every Rust test in the workspace except the desktop app's, from one test archive split over nine sandboxes
  (`cargo nextest archive --workspace --exclude farhelm-desktop`, then a hash partition per sandbox, in the recorder's
  plain command mode because its nextest mode cannot take an archive): 5901 tests, 5900 passed. Runs `d42950f4`,
  `10ab214f`, `239bae3e`, `13916f28`, `efe37aea`, `55d9e4b4`, `830ede08`, `750b573b` and `dac5478e`. Rootless Podman and
  its image were present, so the container tests ran. The one failure, `attribute_compile_contract_matrix` in
  `farhelm-testtrace-macros` (run `10ab214f`), compiles small crates with a nested `cargo`, which the sandboxes do not
  have ("spawn: No such file or directory"); it passed here (below). Two tests reported themselves skipped for their
  substrate, as they do wherever it is missing, and neither concerns these plans: OMP live admission (no Bun on the
  sandboxes) and one tmux test that needs a tmux older than the supported one on `PATH`.

After the landing fixes to upgrade-epochs (the installer's refusal text and the changelog kind), run here: formatting,
`dprint check`, the changelog format check,
`cargo clippy -p farhelm-supervisor -p farhelm --all-targets -- -D warnings`, recorded
`cargo nextest run -p farhelm-supervisor --lib -E 'test(/installation::/)'` (32 passed, run `39b375a6`), recorded
`cargo nextest run -p farhelm-testtrace-macros --test compile_contract` (2 passed, run `05991d87`), and
`test-install-sh.sh` again (601 checks passed). Main after the merges differs from the tested tree only by those fixes
and TODO.md entries other sessions landed meanwhile.

Reused: the executor's own runs for each plan, as their reports list them; this landing reran broader suites rather than
relying on them.

Skipped:

- The rest of the browser suite: the specs above cover the terminal, sizing and stop paths these plans change.
- The desktop smoke: it starts the desktop app and checks its supervisor and terminal, not its updater; the updater's
  refusal is covered by the desktop tests above, and the smoke cannot run a real update from an old release.
- The CentOS provisioning run: it sets up a fresh host, which the helm's new refusal (for hosts already running a
  too-old version) does not affect; the helm's own tests of that refusal ran in the workspace run above.
- The release workflow itself: it runs only on a tag.
- Hosted CI is manual-only and was not dispatched.
