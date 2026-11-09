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
