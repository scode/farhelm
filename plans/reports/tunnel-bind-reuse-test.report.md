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

### Landing

Landed as built. A localhost tunnel must not take connections meant for another program, which Farhelm prevents on the
Mac by turning off address reuse on the tunnel's listener; Linux refuses the conflicting connection either way, so
nothing tested on Linux would have noticed if that protection were removed. A new helm test now reads the option back
from the actual listener and requires it off. Nothing users see changes.

- #2722: pin tunnel listener address reuse.

#### What landed since the stack was based, and whether it interacted

Since the plan was based, main gained the previous landing round, none of which touches tunnels. A fresh-context
reviewer and I read all five plans together; this plan only adds a test. TODO.md conflicted mechanically with entries
the previous round removed; the resolution removes only this plan's entry.

#### Fixes made at landing

None.

#### Checks

This round landed five plans together, in this order: the Codex trust dialog's Quit variant (codex-trust-hint), emoji
widths in the terminal (emoji-streaming-corruption), Claude capture through resident wrappers (claude-wrapper-capture),
progress while Trash deletes (trash-delete-blocking) and a test pinning the tunnel listener's address-reuse option
(tunnel-bind-reuse-test). This round ran the following on all five stacked together on this round's main, before the one
comment fix made at landing (in claude-wrapper-capture):

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, the web-target UI lint
  `cargo clippy -p farhelm-ui --target wasm32-unknown-unknown --features web -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the check that every
  test delay states its reason, `dprint check` on the changed Markdown files,
  `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` (two warnings, both about
  citations main already had), the docs website build, the workspace debug build, the UI's release web build and
  `scripts/check-desktop-assets.sh` (the desktop app bundles exactly the files the web UI asks for, now 28 with the new
  emoji width add-on): all passed.
- The whole Rust workspace, `cargo nextest run --workspace --exclude farhelm-desktop`, with the pinned test runner and
  tmux, four tests at a time, no retries, run `b0c70ae7-5815-4e5d-bed3-20a73a84d43f`: all 4450 passed; 40 more were
  skipped by design (helper entries that only run as child processes of other tests, and tests that need a real
  signed-in Claude or Codex). The UI's desktop-only Rust tests, `cargo nextest run -p farhelm-ui --features desktop`,
  run `a58ba32c-0ad8-4f6c-835a-313a7052cd31`: 959 passed. The tests of the UI's own JavaScript and stylesheet, including
  its animation rules (`node --test` in `crates/farhelm-ui/js-tests`), run `8b354894-f313-4ca4-bb23-28ea92406be7`: 334
  passed.
- `npx playwright test` in `e2e/`, the whole browser suite on Chromium and WebKit, three workers each with its own helm
  and supervisors, no retries, run `e1283879-7e6b-4fcd-99ab-c94dfb2b139d`: 2644 passed, 14 skipped by design (WebKit
  cannot be granted clipboard access), 2 failed, both on Chromium:
  - "expanded setup failure closes the menu below it" timed out waiting for the host actions panel to settle; alone it
    passed 3 of 3 (run `71120673-042e-49b4-8a82-db714b41f755`). No plan in this round touches the hosts panel; one
    failure under a heavily loaded machine is not recorded as a flake.
  - "inline checkout folder setup refreshes discovery and preview and respects host overrides" failed inside the test's
    own wait, which reads the body of every repository discovery response and hit one whose body the browser had already
    discarded. Alone on this stack it failed 1 of 3 (run `f76349e5-6e48-4ebf-b7c3-141e0573b0f7`) and then passed 15 of
    15 (run `49b4a7d3-6340-43dd-9873-b57b51776458`); on this round's main without the five plans it passed 15 of 15 (5
    in run `babb6cc9-3874-4c69-ad5f-d7effd7bf9a1`, 10 in run `a70b9775-910a-47b4-9128-e3f0b60d96ee`). No plan touches
    repository discovery or that test, and 2 failures in 19 runs against 0 in 15 is too few to tie it to the round. A
    possible follow-up, which belongs to no plan in this round and is listed here once for all five: have that wait
    check the response's address before reading its body, so an earlier response it does not need cannot fail the test.
- The desktop smoke test (`scripts/desktop-smoke.sh` with the pinned tmux and its own cargo target directory), run
  `9ba28070-489e-4b87-859f-db818a511d1e`: passed on WebKitGTK under Xvfb. The app's asset handler served every requested
  file, including the new add-on, and the relaunched app attached a terminal to its remembered session.
- After the landing fix, which changes only a test's comment: `cargo fmt -p farhelm-supervisor -- --check` passed.
- Reused: none. Skipped: the doctests (no documentation example changed), real Claude and Codex runs (the reports record
  what their executors observed), and the native Mac app (not runnable here).

The report's checks were the helm's tunnel tests; this round ran the whole Rust workspace with the other four plans.
Nothing in the executor's report became untrue at landing.
