## What this was about

Farhelm's local checks and CI could use different Rust versions. That had already caused CI to reject Clippy lints that
local checks never saw. The repository now pins Rust 1.99.1, the newest stable at execution time, so local checks, CI
and release builds select the same compiler. Updating Rust is a deliberate PR changing that one version.

## Things you should know

The pin includes clippy, rustfmt and the web UI's WASM target. Scripts still add their own cross-compilation targets;
the Mac developer-build script and CentOS check now select the checkout's toolchain even when called from another
directory. Explicit nightly commands for coverage still work. This changes build tooling, with no change to Farhelm's
installed behavior.

The existing CI action already reads the toolchain file, and the generated release jobs use rustup's selection from the
checked-out source. A fresh private rustup installation selected and installed the pin and WASM target on first use. No
second setup action or copy of the version was needed. The Mac release workaround stays: inspection of the checksummed
Rust 1.99.1 archive confirmed that its bundled object-copy tool still expects the LLVM library in the location the
workaround supplies.

## Open questions and possible follow-ups

The next dev release provides the end-to-end confirmation on hosted Linux and Mac runners. This plan did not cut a
release or dispatch CI. Source inspection, independent review and local first-use installation support the
release-selection claim; a hosted release pass is not claimed.

## The PRs

- [#3230](https://github.com/scode/farhelm/pull/3230/changes): pin Rust across local, CI and release builds; open draft,
  head `e0220bdad272`.

## Checks run, reused and skipped

Run and passed on the PR's unchanged Rust sources and pinned toolchain: `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings`, and `cargo clippy -p farhelm --bins -- -D warnings`. The two clippy
configurations cover both test-enabled workspace targets and the shipped binary's configuration.

Run and passed: `dprint check` on the changed Markdown and TOML; `bash -n` on each changed shell script and `shellcheck`
on both; `python3 scripts/check-agents-md.py` (zero errors, one existing warning about a triage record's heading
reference); regeneration and `dist generate --check` with the repository's pinned cargo-dist 0.32.0. Regeneration leaves
the release workflow and dist configuration unchanged. A newer ambient cargo-dist was tried first; its unrelated
rewrites were discarded before pinned verification.

Also checked toolchain selection from the root, the declared WASM target, explicit nightly selection, CI's exact action
source, release target setup and the Mac archive's library layout. No previous session's check results were reused. The
final rebase moved reviewed head `385e86c6466c` onto main's queue bookkeeping, another plan's instructions and an
unrelated TODO entry. The patch comparison is empty, so those checks and the independent review were reused for head
`e0220bdad272`; the tested Rust and build behavior did not change.

Runtime product suites, browser tests, Docker provisioning and desktop launches were skipped because no product code or
test assertions changed. The target-selection edits were checked statically and reviewed; this report does not claim an
actual Mac developer build or CentOS provisioning run. The rehearsal generator changed only explanatory prose, so its
runtime tests were not rerun. No Rust or browser test source changed, so the test-delay checker was not needed.

## Review gate outcome

A fresh gpt-6.1-sol reviewer at high effort reviewed the whole proposed PR against the plan, including its CI and
release selection, script behavior, nightly override and Mac workaround. No findings remained. The commit and PR wording
passed a separate fresh-context cold read. Every touched file received a separate documentation pass.

### Landing

Landed together with xhigh-effort-label (the launcher's extra-high effort label), stacked below it, on a main that had
just gained remote Apple-silicon Mac host setup over SSH. A fresh-context review read the three against each other and
found no interaction. The Mac work installs no Rust and builds nothing itself: it uses the Mac program from a published
release or a directory of locally built programs, and the release's own macOS job will build that program with the
pinned Rust from the next release on. Nothing else on main picks a Rust version against the pin; the explicit nightly
coverage commands keep selecting nightly on purpose. The landing made no fixes, and the executor's report still holds.

#### Things you should know

- No hosted CI or release run has used the pin yet. The release jobs' first Rust command (`rustup target add`) relies on
  rustup installing the pinned compiler by itself. That was confirmed locally with the current rustup (see the checks),
  not on GitHub's runners. The release rehearsal script (`releasing/rehearse-release-gate.py`, which runs the release's
  build and test jobs for a commit without publishing anything) would confirm it there before the next tag.
- CI's caches are keyed on `Cargo.lock` only. After a future toolchain bump, CI will restore caches built by the old
  compiler and rebuild every dependency on each run until the lockfile changes. Adding `rust-toolchain.toml` to those
  cache keys is a small possible follow-up.
- `.github/dist-build-setup.yml` has a step that repairs a library path in the macOS Rust toolchain, needed by a release
  build tool. Its comment now names "1.99.1", and the bump guidance does not say to recheck that step, so the comment
  will go stale at the next bump. A one-line addition to the bump guidance would cover it, if you want it.
- The Alpine installer check that `.agents/run-all-tests.md` lists builds a musl `farhelm`, which needs the musl target
  installed on the pinned toolchain, and the list does not say to add it. On a machine without it, the check fails at
  once with rustup's clear "target not installed" error. The CentOS script, which this plan changed, adds that same
  target to the pinned toolchain, so on a machine that has run it the target is already there.

#### Checks

Both plans were stacked on main after the remote Mac host work (`777f0cdf`) with no conflicts, and checked together on
this development machine during the landing. The pinned Rust 1.99.1 is also this machine's default stable, so the pin
changed no compiler here.

Run now:

- `cargo fmt --all -- --check`, `dprint check`, `python3 releasing/check-changelog.py format` and
  `python3 scripts/check-agents-md.py` passed.
- The pin plan changed how `scripts/devbuild-mac.sh` and `scripts/test-provision-centos.sh` add extra build targets (to
  the pinned toolchain rather than whatever toolchain is the default). `bash -n` and `shellcheck` on both passed.
  `dist generate --check` passed, so the generated release workflow is unchanged.
- `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings` passed under the
  pinned toolchain.
- Recorded `cargo nextest run -p farhelm-ui --lib -E 'test(/launch_composer/)'`: 60 passed, run `8dc58e67`.
- `cargo build`, the release web build, and a recorded browser run of `launcher-enter.spec.ts` and
  `omp-composer.spec.ts` on Chromium and WebKit: 68 passed, run `6ffa8cfc`.
- To check that a release job's first Rust command still works, `rustup target add wasm32-unknown-unknown` was run in a
  directory holding the pin, with an empty private rustup home: rustup 1.29.2 installed the pinned 1.99.1 by itself and
  added the target.

Main after both merges equals the tested tree outside `plans/`.

Skipped, with reasons:

- The CentOS provisioning run and the Alpine installer check: the scripts' only change is which toolchain gets the extra
  target, which the shell checks and the toolchain check above cover. The full runs take a container and a musl build
  each.
- The macOS and arm64 release builds: they run only on GitHub's runners, on a tag or a rehearsal.
- The rest of the browser suite and the workspace's Rust tests: the label change reaches the UI through one function,
  which the launcher tests and specs cover, and the pin changes no compiler here.
- Hosted CI is manual-only and was not dispatched, so no hosted run has used the pin yet.
