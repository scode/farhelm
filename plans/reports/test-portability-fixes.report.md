# Test portability repairs

## What this was about

Twelve regression-test findings concerned assumptions about the machine running the tests. The checks could report
success without observing a stopped client on macOS, reject correct folder identities when temporary directories contain
symlinks, or fail before reaching the behavior they were meant to check. Other checks assumed GNU command-line tools or
4096-byte memory pages. The maintainer chose contained test repairs for all twelve, with any unexpectedly large repair
left for later triage.

All twelve repairs stayed within that scope. Farhelm's runtime behavior is unchanged. The work strengthens the evidence
for supervisor client cleanup, checkout ownership and recovery, folder browsing, upload memory bounds, and provisioning
cleanup.

## Things you should know

The twelve outcomes share these repairs:

- Three terminal-output cleanup checks: the supervisor reads a session's terminal through a tmux client process. Tests
  previously treated a missing Linux process-directory entry as proof that this client had stopped, which could pass
  without observing anything on macOS. They now first observe the client alive and check its disappearance with a
  portable process probe. The three boundaries remain distinct: orderly shutdown returns only after cleanup; a client
  that closes its output but stays alive is removed before its replacement opens; and shutdown during replacement
  startup removes the newly returned client before returning.
- Checkout validation: constructing an invalid UTF-8 directory name failed on APFS before the other validation checks
  could run. Only that fixture is omitted on macOS; relative-root, label and size refusals still run.
- Upload memory bounds: converting resident pages with a fixed 4096-byte multiplier undercounted memory on large-page
  Linux systems. The check now uses the kernel's reported page size.
- Checkout creation-time ownership: four tests could mistake a failed independent creation-time probe for an unsupported
  filesystem and skip their assertions. The probe now uses each platform's command syntax, fails on command or parsing
  errors, and permits a capability skip only after a successful observation that creation time is unavailable.
- Three provisioning checks: failed uploads must remove partial temporary files; a new upload must sweep older orphaned
  temporary files; and installation must reject payload tampering after upload while preserving the installed binary.
  Their local Linux remote-shell stand-ins need GNU metadata tools, so native macOS failed before reaching these
  operations. These tests now run only on Linux, where all three checks remain active.
- Retrying session creation with a home-relative folder: once creation succeeds, retrying the same request must replay
  that success even if the home location later becomes unusable. The test confused the expanded path shown to the user
  with the resolved folder identity when the temporary home path contained a symlink. It now checks the preserved
  display spelling and the canonical folder separately.
- Recovering a fresh checkout after a lost success response: retrying after checkout settings change and the helm
  restarts must recover the original result. The test compared the recorded canonical checkout roots with unresolved
  temporary-folder spellings. Both root comparisons now resolve the fixture paths before comparing; the recovery
  assertions remain intact.
- Browsing a home-relative folder: the supervisor returns its canonical parent directory. The test now resolves the
  expected home path too, so a symlink in the temporary home location does not reject a correct browse result.

Validation ran on Linux. macOS behavior was checked by reading the code only. This work does not make the entire Rust
end-to-end harness portable or repair unrelated platform assumptions.

## Open questions and possible follow-ups

None. No outcome was dropped by the complexity gate or discarded as already fixed. Whole-suite portability and unrelated
RSS or procfs tests remain outside this plan.

## The PRs

- [PR #1834](https://github.com/scode/farhelm/pull/1834/changes): one draft containing all twelve test repairs and their
  ledger and feedback-queue cleanup, based on main; final head `fca0683b8bbe879bf6ab54eb109f59acdc9a97d6`.

## Checks run, reused and skipped

Run on the unchanged source patch based on main at `4c4ec596`:

- Thirteen changed supervisor and helm cases passed through the recorder with pinned nextest and tmux, four slots and
  zero retries; run `5a5dc0f0-0f30-42e1-bd2d-6a8a0e1b06a8`. The retained output contains no runtime capability skip. The
  three client-lifetime tests establish their live premise before checking disappearance. The upload memory check passed
  with the kernel reporting 4096-byte pages, and the source calls the system page-size query rather than assuming that
  value.
- The two changed replay and checkout-recovery end-to-end cases passed through
  `cargo nextest run -p farhelm --test e2e -E <two changed cases>` with the same recorder and runner policy; run
  `3a1c1ca6-0d3e-47da-9eb2-5a61ad73f759`. Fixture setup also passed, and there were no runtime capability skips.
- With only the invoked command's lookup path changed to a deliberately failing `stat`, all four creation-time tests
  failed at the independent probe, rather than printing SKIPPED; expected-failure run
  `ae0a261b-19e8-4213-8f2d-53feaf347a76`. Restoring the normal lookup path produced four passes and no runtime
  capability skips; run `a406bec4-05aa-47de-9d6f-5d6f94ff7c95`. Both used
  `cargo nextest run -p farhelm-supervisor --lib -E <four changed creation-time cases>` through the recorder, four slots
  and zero retries. No test changed its own process environment, and no source mutation was needed.
- Rust formatting and `cargo clippy --all-targets -- -D warnings` passed. The isolated delay checker found 283 delays
  and zero unexplained ones. Formatting of the changed ledger and feedback index passed.

The listed checks remain applicable after the final rebase onto main at `c0953fb8` and the twelve PR URL additions. The
intervening upstream diff only removed an approved plan's tracking files; the test-source patch remained byte-identical,
with no code, specification or dependency changes. Those results were reused without another runtime run. Broader Rust,
browser, installer and desktop execution was skipped because this patch changes only the named tests and their
bookkeeping; the focused cases exercise the changed contracts. Native macOS execution and a large-page Linux kernel were
unavailable; only code reading covers those platform variants.

## The review gate's outcome

The required fresh-context source review, requested on gpt-6.1-sol at high effort, found no remaining correctness, scope
or idiom issues after checking all twelve completion criteria and the full test-authoring checklist. The commit and PR
wording cold read, requested on gpt-6.1-sol at medium effort, matched the intended meaning and found no convention
violations. Initial task delivery was unreadable; task-file instructions were reconciled before accepting the findings
artifacts. Actual native model attribution and usage counters are not exposed by the harness.

Implementation and investigation were done by the executor under the plan's no-workhorse rule; only the required reviews
were delegated. The PR stays a draft; the executor neither marks it ready nor merges it.
