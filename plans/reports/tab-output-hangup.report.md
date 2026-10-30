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

Both main validation selections had a failure in an existing test, described below. All new regression tests passed; the
failed commands are retained and are not reported as passing gates. The independent reviewer found no concrete
patch-related cause or supported correction for those failures.

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
  `c01ba197ed4971444da3035ff1be689d05313a10`.

## Checks run, reused and skipped

All runtime checks below used the recorder, nextest 0.9.148, pinned tmux 3.7c, four global slots and zero retries.
Selections outside the filters are not passes. Retained output was complete and contained no literal runtime `SKIPPED`
notices. Successful individual cases inside failed commands are observations, not a successful selection verdict.

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
- Source coverage: supervisor checks used the implementation diff over `9ad2a17f`; integration used the same code over
  `3abda2d9`. Final head rebased onto `d3d67f52`. The intervening main diffs only changed plan bookkeeping and another
  plan's report; the tested product and test code is unchanged. No runtime result from an earlier plan was reused.
- Skipped the RSS measurement because no buffering, allocation or flow-control policy changed and shared-host memory
  noise would not test the cleanup guards. Skipped browser, desktop, installer and full-workspace suites because the
  changed behavior is in supervisor cleanup and diagnostics, covered by the targeted backend checks. No live
  installation was changed.

## Review gate outcome

The required fresh gpt-6.1-sol/high adversarial reviewer reviewed correctness, design, idioms and the complete
test-authoring checklist. It found one diagnostic bug: a byte cap could split a PID and invent a different member in the
scope snapshot. The parser now discards that partial record, and the reviewer accepted the focused regression and
correction. No finding was rejected. Scope reassessment found the existing guards and bounded diagnostics sufficient,
without a new lifecycle subsystem. Separate review of both validation failures found no concrete patch defect and
recommended retaining the observations without assertion weakening or broader hunting. The commit/PR wording passed an
independent cold read.
