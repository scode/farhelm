## What this was about

A localhost tunnel must not take connections intended for another program. Farhelm already prevents the macOS
address-reuse behavior that could cause that, but Linux rejects the conflicting bind either way, so its behavioral tests
could keep passing if the protection were accidentally removed. The maintainer requested a test that reads the actual
listener's socket option, with one local mutation to prove that the test detects the regression.

The new test first establishes that the returned listener is on IPv4 loopback, then reads `SO_REUSEADDR` and requires
zero. It checks the syscall and returned value size, and explains the platform difference in its documentation. The
matching TODO entry is removed.

## Things you should know

Product behavior and dependencies are unchanged. The test uses the helm's existing libc dependency and checks the
returned listener, so replacing its construction with a convenience bind that enables reuse would also be caught. It
checks only the address-reuse option required by the specification.

The deliberate Linux mutation enabled address reuse and failed specifically because the readback was 1 instead of 0. The
production flag was immediately restored before final validation, review or publication. The final tunnel suite passed.
This is Linux execution evidence; no macOS run is claimed.

## Open questions and possible follow-ups

None. There was no scope departure or unresolved finding.

## The PRs

- [#2722](https://github.com/scode/farhelm/pull/2722/changes) — draft, `test: pin tunnel listener address reuse`; one
  test and its TODO removal.

## Checks run, reused and skipped

- Ran the intentional mutation through the recorder with
  `cargo nextest run -p farhelm-helm --lib -E 'test(=tunnels::tests::tunnel_listener_binds_without_address_reuse)'`: run
  `69b2c8c8-f760-43dd-814f-80465b370ca3`, one expected failure at the option assertion. This proves detection, not a
  passing product check. Its evidence remains retained separately.
- After restoring the flag, ran the recorder with `cargo nextest run -p farhelm-helm --lib -E 'test(tunnels::)'`: run
  `a343e8f5-c870-426b-993b-2eb3823adf83`, all 20 selected tests passed; 1,299 were selected out. Complete report,
  untruncated output with EOF, no runtime skip markers and clean process cleanup. Both runs used pinned nextest 0.9.148,
  four slots, zero retries and tmux mode `none`: these tests use owned sockets and scripted peers, without tmux or
  systemd. The final run covers the two-file change on base `11fa6dbf`.
- Passed `cargo clippy -p farhelm-helm --all-targets -- -D warnings` to check the new test and its compilation contexts.
- Passed `cargo fmt --all -- --check` and the isolated interpreter's `python -B scripts/check-test-sleeps.py` (323
  delays, zero missing explanations). These source checks ran during the deliberate mutation and are reused: restoring
  its boolean leaves their formatting and delay coverage intact.
- Passed `dprint check TODO.md` and `git diff --check` on the final change.
- Reused the targeted runtime, Clippy and source-check evidence after rebasing onto `ac20773b`. The complete intervening
  diff only changes an unrelated supervisor create-request test to disable a host scope probe, with accompanying flake
  evidence and helper documentation. It changes no tunnel behavior or test dependency, and the reviewed two-file patch
  is unchanged. No additional runtime run was needed.
- Skipped broader runtime, browser and installer suites because this is a unit-test-only change with no product behavior
  or shared harness change. Skipped macOS execution; direct option inspection plus the Linux mutation meets the agreed
  plan. No instruction files changed, so the agent-instructions check was unnecessary.

## The review gate's outcome

The required single fresh-context gpt-6.1-sol reviewer at high effort found no issues in correctness, design, idiomatic
code or the test-authoring contracts. It reviewed the complete two-file snapshot, the listener implementation,
neighboring tests and the specification. There were no findings to reject. The separate commit-message cold read matched
the intended motivation and found no contradicted claims or convention violations. No scope reassessment was needed.
