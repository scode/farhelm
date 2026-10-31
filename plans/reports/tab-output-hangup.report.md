## Since the last review

Only draft #2846 changed; no PR was added or dropped. Landing review caught a regression in the new terminal-identity
guard: ordinary Restart failed when the session's folder contained `}`, because tmux parsed that unquoted character as
command syntax. Every word in the nested relaunch and related guarded commands now uses the existing always-single-quote
encoder, and credential redaction matches the resulting text.

A new real-tmux regression restarts an ordinary session in a folder named `left}right`. It requires the same terminal, a
different process, the exact working folder, and a unique response to input through a fresh attachment. Retained
scrollback cannot supply that response. The original incident remains unattributed; the agreed audit-and-diagnostics
fallback is unchanged.

## What this was about

A command run directly in Bash in an ordinary terminal tab repeatedly printed "Terminated" after two to three minutes of
heavy output. The tab and Bash survived, while other tabs were being opened and closed. The maintainer asked for
signal-level attribution, an audit of everything that can end a tab, and diagnostics for a recurrence. The agreed
fallback was to deliver the audit fixes and evidence if the incident could not be reproduced.

The incident remains unexplained. Two corrected baseline investigations ran beyond three minutes and completed
naturally, including one with explicit closes and automatic cleanup of other tabs. Neither attributes the original
termination to Farhelm or to another program. This delivery uses the fallback: one draft PR fixes independently
identified cleanup hazards, adds diagnostics, and removes the planned TODO entry. No cause-specific second PR was made.

## Things you should know

Automatic cleanup could try to stop a command that was still running if its tab became live again before cleanup reached
it. It now checks again that every pane in that tab's window is dead before signalling anything. Removing or restarting
an old terminal could also act on a different surviving terminal after the original pane disappeared: tmux can resolve
that vanished target to a surviving terminal. The destructive command now checks terminal identity in the same tmux
operation and refuses that fallback. These hazards were found independently; neither is established as the reported
incident's cause.

The audit covered Close, automatic cleanup after shell exit, Delete, Stop, Restart, old-window removal and failed-open
cleanup. Ordinary Stop and Restart still select the agent, while Delete selects the session and Close selects its tab.
No timeout or output-slowness path was found that initiates tab termination; a stalled viewer is detached. Two specified
exceptions remain: an Open that fails discards its unfinished tab, and stopping a containerized session ends that
container's tabs. Signal selection, escalation order and flow-control settings are unchanged.

Pause logs now identify the terminal and whether pane death requested the pause. A flow-control pause can already be
queued then, so this is request context rather than exclusive causal proof. Automatic cleanup records the tab, pane and
tmux exit status or signal; close logs record their reason. Process sweeps log up to 32 successful identity-checked
signal syscall recipients, their total and truncation. Scope signals instead log a capped best-effort snapshot of direct
cgroup members, with unavailable/truncated flags. That snapshot can race delivery and omit nested members; it cannot
identify exact systemd signal recipients by itself.

The earlier execution's two main validation selections each had a failure in an existing test, described below. Those
failed commands remain retained and are not reported as passing gates. The original independent reviewer found no
concrete patch-related cause or supported correction for those failures. This round's required package and module checks
are reported separately below.

## Open questions and possible follow-ups

The original command and an attributable recurrence are still needed to settle the incident. The investigations used
synthetic output through a real tab PTY and integration client on Linux with systemd, rather than the original command,
a browser or the desktop renderer. They did not deliberately stall that viewer. Caught TERM/HUP/INT/QUIT/PIPE signals
could be attributed with a calibrated signal handler; SIGKILL cannot be caught. Every investigated producer completed
naturally, so there was no unexpected sender to identify.

Two test-observable limits remain. One test expects an exited command's final error line in the terminal capture, but
does not first establish that the line reached tmux's rendered grid. Its failure cannot distinguish a capture defect
from that missing observation premise. Another expects a briefly paused viewer to resume without clearing and rebuilding
its terminal from history. It rejects every such reset for ten seconds after resume, including a possible later recovery
from another output stall, without retaining enough context to attribute the reset. Narrow observations and prior
history do not settle the current failures' causes. Improving those observables is separate from changing flow control
or weakening either assertion.

## The PRs

- [#2846: keep tab cleanup on its own terminal](https://github.com/scode/farhelm/pull/2846/changes), draft head
  `c518c5da1acbf2db43683a2b38359c609cd14e89`.

## Checks run, reused and skipped

All runtime checks below used the recorder, nextest 0.9.148, pinned tmux 3.7c, four global slots and zero retries.
Selections outside the filters are not passes. Retained output was complete and contained no literal runtime `SKIPPED`
notices. Successful individual cases inside failed commands are observations, not a successful selection verdict.

This round:

- `cargo nextest run -p farhelm-supervisor`: recorder `90238937-400b-40f8-b431-f5285237a7e6` passed all 1,386 executed
  tests; five helper tests were intentionally ignored.
- `cargo nextest run -p farhelm --test e2e -E 'test(restart_with_resume::) or test(restart_under_concurrency::) or test(terminal_tabs::)'`:
  recorder `df945619-d3f3-490b-88d7-58fdacfc4524` passed all 60 selected tests plus fixture setup; 350 were outside the
  selection. This includes the final ordinary Restart regression in the closing-brace directory.
- Earlier focused recorder `395a7948-32c1-45a7-94be-f08209e2b09d` passed the new Restart case and the real-driver
  directory and credential-redaction tests. The redundant post-Restart readiness wait was subsequently removed; the
  complete module run above covers that final version.
- Both required runs captured complete source identity over `49cdcb148b33c791b99de0d051d6d7dbd6a9ebc3` with tracked
  follow-up edits, fingerprint `f04cfb49fbd635489d017df5c974ee583cb8e399e22c2e8f781239508cf57fc6`. Reports and output
  were complete, without runtime skips or forced cleanup.
- Passed `cargo clippy -p farhelm-supervisor --all-targets -- -D warnings`, `cargo fmt --all -- --check`, Markdown
  formatting on the PR's documents, `python3 releasing/check-changelog.py format`, and `git diff --check`. The isolated
  interpreter's `python -B scripts/check-test-sleeps.py` passed with 327 inspected delays and zero unannotated delays.
- Rebased onto `1c9734e37f8444ec6160abfc2a50362adb8ae0b5` after inspecting the complete intervening diff. It only
  changed documentation, screenshot fixtures and queue bookkeeping. The complete PR diff before and after rebase
  compared byte-for-byte equal, so the runtime, lint and review coverage still applies. The new documentation does not
  change the session cleanup or Restart contracts.
- Did not repeat the several-minute investigation: the decision requested the quoting correction, real Restart
  regression and package/module checks, while retaining the original no-reproduction fallback. Browser, desktop,
  installer and whole-workspace runs add no uncovered integration risk to this backend correction. The earlier
  backpressure failures below remain disclosed; no assertion or flow-control behavior was weakened.

Earlier execution retained and reused for the original investigation and validation limits:

- Original-baseline investigation at `9ad2a17f`: recorder `2ef679c7-5d3a-4610-9d13-b497c5464bb5` passed a quiet
  194-second run with 175,424 lines and about 22.6 MB delivered. Recorder `da07a64e-d83f-4567-add3-4e653d754397` passed
  a 197-second churn observation with 196,032 lines and about 25.3 MB, 22 explicit closes and 20 physically verified
  automatic window removals across the same and another session. Both producers completed naturally and their original
  Bash/tab survived. Temporary investigation tests were removed, rather than adding minutes to the permanent suite.
- Earlier recorder `1aa47382-1af0-4912-889c-94612a4d5164` passed TERM-sender calibration and a quiet output run;
  `1ad24594-3087-4bed-9838-cf8ef053d62e` passed initial churn. Those fixtures omitted the serving loop that runs
  automatic cleanup, so their listings did not prove physical reaping. The corrected runs above supplied that premise.
- Supervisor selection:
  `cargo nextest run -p farhelm-supervisor --lib -E 'test(scope::tests::) | test(service::sweep::tests::) | test(/service::ticker::tests::.*tab/) | test(tmux::tests::)'`.
  Recorder `e56e96d5-cd35-4878-9e59-0ea5e72d3b61` failed: 127 passed and the existing
  `launch_excerpt_numeric_height_accepts_a_tall_zero_history_pane` failed because its complaint was absent from both the
  excerpt and independent pane-grid diagnostic. New death-recheck, stale-target, credential-redaction, signal-log and
  capped-PID tests passed. Exact retry `d9de080b-3559-4cf6-a007-af7c34c33c86` passed, as did all five exact attempts in
  batch `d3340dbd-53dc-4258-8a81-f77756e942f3`. Those six observations do not erase or classify the initial failure.
- Integration selection:
  `cargo nextest run -p farhelm --test e2e -E 'test(terminal_tabs::) | (test(terminal_backpressure::) & not test(memory_stays_flat_while_a_viewer_is_stalled))'`.
  Recorder `4872e9e3-a04e-431f-b3cd-b8de4c20c113` failed: 50 passed and `shallow_pause_resumes_without_reset_or_replay`
  observed a reset. All three exact attempts in batch `395218bb-91eb-4cc7-b598-c0aeb22ae1bb` failed the same assertion.
  The fixture remained live; key pause/reset trace events were evicted. The same assertion has documented failures
  predating this patch, but that alone does not prove the current cause.
- Exact original-baseline comparison `05dc245a-cc66-4e46-8846-97696c6d16bc`, at unchanged tracked source `9ad2a17f`,
  also failed the shallow-pause reset assertion. That establishes the observable occurs without this patch; it does not
  establish that every failure has the same cause.
- Passed `cargo clippy -p farhelm-supervisor --all-targets -- -D warnings`, `cargo fmt --all -- --check`, configured
  Markdown formatting on changed documents, and `python3 releasing/check-changelog.py format`.
  `python -B scripts/check-test-sleeps.py` passed with 327 inspected delays and none lacking a rationale, after two
  earlier attempts exceeded its 60-second source-check budget.
- Earlier source coverage: supervisor checks used the implementation diff over `9ad2a17f`; integration used the same
  code over `3abda2d9`. That execution's final head rebased onto `d3d67f52`. The intervening main diffs only changed
  plan bookkeeping and another plan's report; the tested product and test code is unchanged. No runtime result from an
  earlier plan was reused.
- Skipped the RSS measurement because no buffering, allocation or flow-control policy changed and shared-host memory
  noise would not test the cleanup guards. Skipped browser, desktop, installer and full-workspace suites because the
  changed behavior is in supervisor cleanup and diagnostics, covered by the targeted backend checks. No live
  installation was changed.

## Review gate outcome

This round's required fresh gpt-6.1-sol/high adversarial reviewer passed the combined PR, including every nested command
builder, matching credential redaction and the real Restart regression. Its only clarity note was accepted: the test now
explains that retained readiness banners cannot establish the replacement agent's liveness; its new process and unique
input response do. The corrected test was re-read and passed with no findings. The existing commit and PR wording also
passed a new independent cold read.

In the earlier execution, the required fresh gpt-6.1-sol/high adversarial reviewer reviewed correctness, design, idioms
and the complete test-authoring checklist. It found one diagnostic bug: a byte cap could split a PID and invent a
different member in the scope snapshot. The parser now discards that partial record, and the reviewer accepted the
focused regression and correction. No finding was rejected. Scope reassessment found the existing guards and bounded
diagnostics sufficient, without a new lifecycle subsystem. Separate review of both validation failures found no concrete
patch defect and recommended retaining the observations without assertion weakening or broader hunting. The commit/PR
wording passed an independent cold read.

### Landing

Landed on its second attempt, with no fixes at landing. The plan could not find what terminated the command in the
reported incident; it delivers the agreed fallback: terminal-tab cleanup now checks a terminal's identity before it
kills or relaunches anything, so a cleanup can no longer hit the wrong terminal, and the log now records more context
(pauses and the session's processes) toward finding the cause if it happens again, though not proof of it.

- #2846: the identity checks on tab cleanup and Restart's relaunch, the diagnostics, tests, the specification text and a
  changelog entry.

The first landing attempt (2026-10-30) sent this plan back because an ordinary Restart's relaunch would fail with a tmux
syntax error for a session whose folder path contains `}`. The redelivery single-quotes every word of the commands it
nests inside tmux (the environment, the folder, the window command, the window kill and the reads of the terminal's
state), and adds an end-to-end test that restarts a live session in a folder ending in `left}right` through that
relaunch, against a real tmux.

#### What landed since the stack was based, and whether it interacted

Nothing that landed on main since touches tmux or tab cleanup. sqlite-wal landed right before this plan in the same
round; the two change separate code.

#### Things you should know

- One tmux quirk quoting cannot fix: inside a quoted word, tmux drops spaces and tabs after a newline, and a newline
  followed by `#` starts a comment. A session folder whose name contains a newline followed by a space, tab or `#` would
  restart in the wrong folder (tmux falls back to the home directory). Creating such a session still works; refusing
  such paths, or writing the newline differently, would be a follow-up.
- Before each kill step of Stop, Delete and Restart's cleanup, the supervisor now asks the systemd user manager which
  processes the session holds, only to log them (up to 5 seconds on a slow manager); and closing a tab fails if tmux's
  pane listing fails.

#### Checks

This round landed sqlite-wal and tab-output-hangup, stacked in that order on main `e9095ef8`. On that stack:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, the web-target UI lint,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the check that every
  test delay states its reason (`python -B scripts/check-test-sleeps.py`, 327 delays, none without a reason),
  `dprint check`, `python3 releasing/check-changelog.py format`, the workspace debug build and the UI's release web
  build: all passed.
- The UI's Rust tests, `cargo nextest run -p farhelm-ui`, run `a47ccf43-6220-4fa1-a441-918dd35fd486`: all 833 passed;
  its desktop-build tests, `cargo nextest run -p farhelm-ui --features desktop`, run
  `19b4cecc-69ec-47c1-9d7b-1549b0f5d95c`: all 1016 passed. The UI's JavaScript tests (`node --test` in
  `crates/farhelm-ui/js-tests`), run `f937cb3a-908c-4912-aa54-16e68f300565`: all 383 passed.
- The whole Rust workspace, `cargo nextest run --workspace --exclude farhelm-desktop`, with the pinned test runner and
  tmux, no retries, on a machine with rootless Podman, run `c7309da5-bfd5-494f-a120-8ee35001698e`: all 4612 passed, 41
  left out by the test runner (tests marked ignored, such as real-agent captures).
- The whole browser suite on Chromium and WebKit, on ten Tensorlake sandboxes at the maintainer's request (cloud virtual
  machines, not the usual development machine; each booted the usual three test stacks and ran its share of the suite's
  218 projects through the recorder's generic mode, `python3 scripts/record-test-run.py --kind development ...` running
  `npx playwright test --project=<engine>-<spec> ... --retries=0`, with the binaries and web bundle built locally from
  this stack; Node 24.16 there, against 24.20 locally). It took 16 minutes. Across the ten runs
  (`0c8864cb-3172-4c93-b299-459fe2be756c`, `8b206f4c-20eb-4e1f-9502-c77dd9f59481`,
  `0beabe85-c643-48b7-a4c0-d5f75e2f686a`, `bc37b0d7-481a-4b8d-b693-76657dab38a1`,
  `7df60eb0-647f-4a3e-b2bf-db779a98a056`, `b4321572-5693-4867-a361-fcdc51e4d2cc`,
  `a3114930-73dc-4590-8467-de75e94d1d93`, `a4a4ad3b-56c1-426f-8377-149f23f29d12`,
  `8ac5a34e-85cd-4f2c-86c5-ac9756d7ba46`, `3419f1ed-b945-43c4-96ab-3403b80ca8c0`; only the two failed runs' records were
  kept), 2846 passed, 14 skipped by design, 2 failed, both on WebKit. Re-run three times each on the same build, also on
  sandboxes, with `-g` and `--repeat-each=3` (runs `f075c442-c5a9-401a-a226-ec07dffc3f8e` and
  `7b4a27da-01e2-4721-aef3-c5c6c201ae50`). "unchanged launcher tab, harness, permissions and default model retain the
  retry › selected Agent Home" (launcher Enter: the Launch button did not take keyboard focus within 5 seconds) passed 3
  of 3; it is a first observation with no known cause, its record is kept, and it is not logged as a flake on one
  observation. "the dialog refuses blank and over-long messages without sending" (the Send feedback dialog) is a
  recorded open flake, but this time it failed on the Send button's background color (dark where light was expected,
  which fits the record's color-scheme hypothesis) and once more in the three runs alone, both new to its record.
  Neither plan touches either.
- Reused: nothing. Not run: the doctests (the stack adds no doc examples), `cargo check -p farhelm-desktop` and the
  desktop smoke tests (only a message changed in the desktop code, and the desktop-feature UI build was checked above),
  the docs website build (no page changed) and the native Mac app (not runnable on Linux).

The report's checks were the executor's selected runs; this landing ran the checks below, in which the two tests the
report disclosed as failing during execution passed. Nothing in the executor's report became untrue at landing.
