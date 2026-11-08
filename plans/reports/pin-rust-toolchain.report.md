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
